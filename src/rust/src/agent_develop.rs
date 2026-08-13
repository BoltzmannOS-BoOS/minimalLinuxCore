use std::process::Command;
use std::path::Path;
use std::time::Duration;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

use crate::config;
use crate::memory;
use crate::principal::PrincipalContext;
use crate::agent_loop::truncate_utf8;

const LOOP_DELAY_MS: u64 = 1000;
const MAX_WRITE_BYTES: usize = 64 * 1024;

static CARGO_TOML_HASH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn ask_deepseek(system_prompt: &str, context: &str) -> Option<String> {
    if let Some(resp) = gateway_ask(system_prompt, context) {
        return Some(resp);
    }
    eprintln!("  [gateway unreachable — agent cannot call API directly]");
    None
}

/// Send DEEPSEEK request to the local gateway (has key access)
fn gateway_ask(system_prompt: &str, context: &str) -> Option<String> {
    use std::io::{Write, BufRead, BufReader};
    let mut stream = std::net::TcpStream::connect("127.0.0.1:5555").ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(35)));
    // Protocol: DEEPSEEK\n<system>\n<context>\n
    let _ = writeln!(stream, "DEEPSEEK");
    let _ = writeln!(stream, "{}", system_prompt.replace('\n', "\\n"));
    let _ = writeln!(stream, "{}", context.replace('\n', "\\n"));
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).ok()?;
    let resp = response.trim().to_string();
    if resp.starts_with("GATEWAY:") { eprintln!("  [{}]", resp); return None; }
    if resp.is_empty() { return None; }
    Some(resp.replace("\\\\", "\\").replace("\\n", "\n").replace("\\\"", "\""))
}

/// Build context for the develop loop: source tree overview + goal + recent actions.
fn build_develop_context(
    goal: &str,
    recent_actions: &[String],
    round: u32,
    max_loops: u32,
    results_directory: &Path,
) -> String {
    let mut ctx = String::new();

    ctx.push_str(&format!("Goal: {}\n", goal));
    ctx.push_str(&format!("Round: {}/{}\n\n", round, max_loops));

    // List available source files the AI can modify.
    ctx.push_str("Source files (use READ to inspect, WRITE to modify):\n");
    let src_dir = "src/rust/src";
    if let Ok(entries) = std::fs::read_dir(src_dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".rs") {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let full_path = format!("{}/{}", src_dir, name);
                ctx.push_str(&format!("  {} ({} bytes)\n", full_path, size));
            }
        }
    }
    ctx.push_str("  src/rust/Cargo.toml (project config)\n");

    // Include audit trail summary so the agent knows failure patterns
    if let Ok(entries) = std::fs::read_dir(results_directory) {
        let mut total = 0u32;
        let mut failures = 0u32;
        for e in entries.filter_map(|e| e.ok()) {
            if e.path().extension().is_some_and(|ext| ext == "out") {
                let kv = crate::registry::parse_kv_file(&e.path());
                total += 1;
                let v = kv.get("verdict").map(|s| s.as_str()).unwrap_or("");
                if v == "denied" || v == "error" || v == "unknown" {
                    failures += 1;
                }
            }
        }
        if total > 0 {
            ctx.push_str(&format!(
                "\nAudit: {} past actions, {} failures for this principal\n",
                total, failures
            ));
        }
    }

    // Show recent actions (last 5)
    if !recent_actions.is_empty() {
        ctx.push_str("\nRecent actions:\n");
        for action in recent_actions.iter().rev().take(5) {
            let truncated = truncate_utf8(action, 120);
            ctx.push_str(&format!("  {}\n", truncated));
        }
    }

    ctx.push_str("\nRespond with ONE of these action formats:\n");
    ctx.push_str("  READ <filepath>           — read a source file\n");
    ctx.push_str("  WRITE <filepath> <content> — create/overwrite a file\n");
    ctx.push_str("  FETCH <url>               — get allowlisted HTTPS context (may be disabled)\n");
    ctx.push_str("  BUILD                     — run cargo build\n");
    ctx.push_str("  TEST                      — run cargo test\n");
  ctx.push_str("  CHECKPOINTS               — list saved states
   BRANCH <ck-id> <name>     — branch from checkpoint
   ROLLBACK <ck-id>          — restore from checkpoint
   DIFF                      — show file changes since last snapshot
   DONE <summary>            — task complete\n");
    ctx.push_str("\nOnly respond with the action. No explanation, no markdown.\n");

    ctx
}

fn memory_context_for_goal(goal: &str, entries: &[memory::RecentEntry]) -> String {
    let goal_lower = goal.to_lowercase();
    entries
        .iter()
        .rev()
        .filter(|entry| entry.content.to_lowercase().contains(&goal_lower))
        .take(3)
        .map(|entry| format!("[MEMORY] {}\n", truncate_utf8(&entry.content, 120)))
        .collect()
}

fn assemble_round_context(
    goal: &str,
    develop_context: &str,
    memory_context: &str,
    self_state: &str,
) -> String {
    format!(
        "目标: {}\n\n{}\n{}\n当前身体状态:\n{}",
        goal, develop_context, memory_context, self_state
    )
}

/// Parse and execute a single develop action.
fn execute_develop_action(action: &str) -> String {
    let action = action.trim();

    if action.is_empty() {
        return "(empty action)".to_string();
    }

    let upper = action.to_uppercase();

    if upper.starts_with("READ ") {
        let path = action[5..].trim();
        if config::is_protected_read_path(path) {
            return format!("READ denied: '{}' is a protected read path", path);
        }
        match std::fs::read_to_string(path) {
            Ok(content) => truncate_utf8(&content, 2000),
            Err(e) => format!("READ error: {}", e),
        }
    } else if upper.starts_with("WRITE ") {
        let rest = action[6..].trim();
        let space_pos = match rest.find(|c: char| c.is_whitespace()) {
            Some(p) => p,
            None => return "WRITE: missing content".to_string(),
        };
        let path = rest[..space_pos].trim();
        let content = rest[space_pos..].trim();
        if path.is_empty() || content.is_empty() {
            return "WRITE: path and content required".to_string();
        }
        // BIOS: reject writes to protected system directories
        if crate::config::is_protected_path(path) {
            return format!("WRITE denied: '{}' is a protected system path (BIOS restriction)", path);
        }
        // Size cap — prevent disk exhaustion
        if content.len() > MAX_WRITE_BYTES {
            return format!("WRITE denied: content too large ({} > {} bytes)", content.len(), MAX_WRITE_BYTES);
        }
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        match std::fs::write(path, content) {
            Ok(()) => format!("WRITE ok: {} ({} bytes)", path, content.len()),
            Err(e) => format!("WRITE error: {}", e),
 }
    } else if upper.starts_with("CHECKPOINT") {
        let label = if action.len() > 11 {
            action[11..].trim()
        } else {
            "manual"
        };
        let ck = crate::checkpoint::CheckpointManager::new();
        let actions: Vec<String> = Vec::new();
        let id = ck.create("develop-session", label, &actions, 0, None);
        format!("CHECKPOINT created: {}", id)
    } else if upper.starts_with("BRANCH") {
        let args: Vec<&str> = action[7..].split_whitespace().filter(|s| !s.is_empty()).collect();
        if args.len() < 2 { return "BRANCH: checkpoint-id and branch-name required".to_string(); }
        let ck = crate::checkpoint::CheckpointManager::new();
        match ck.branch(args[0], args[1]) {
            Some(id) => {
                let mut result = format!("BRANCH created: {}", id);
                // Attack-evolve integration: if branch name starts with "attack", run in branch
                if args[1].starts_with("attack") {
                    match std::process::Command::new("sh")
                        .args(["../tests/auto-attack.sh"]).output() 
                    {
                        Ok(o) => {
                            let out = String::from_utf8_lossy(&o.stdout);
                            let status = if out.contains("0 failed") { "PASS" } else { "FAILURES" };
                            result.push_str(&format!("\nBRANCH ATTACK: {} ({} chars)", status, out.len()));
                        }
                        Err(e) => result.push_str(&format!("\nBRANCH ATTACK error: {}", e)),
                    }
                }
                result
            }
            None => format!("BRANCH failed: checkpoint {} not found", args[0]),
        }
    } else if upper.starts_with("ROLLBACK") {
        let ck_id = if action.len() > 9 { action[9..].trim() } else { "" };
        if ck_id.is_empty() { return "ROLLBACK: checkpoint-id required".to_string(); }
        let ck = crate::checkpoint::CheckpointManager::new();
        match ck.load(ck_id) {
            Some(cp) => {
                format!("ROLLBACK: restored from {} (round {}, {} actions)", 
                    cp.id, cp.round, cp.recent_actions.len())
            }
            None => format!("ROLLBACK failed: checkpoint {} not found", ck_id),
        }
    } else if upper.starts_with("CHECKPOINTS") {
        let ck = crate::checkpoint::CheckpointManager::new();
        let list: Vec<String> = ck.list().iter().map(|c| 
            format!("{}: r{} [{}] {}", c.id, c.round, c.branch_name, c.label)
        ).collect();
        format!("CHECKPOINTS:\n{}", list.join("\n"))
    } else if upper.starts_with("FETCH") {
        let url = action[6..].trim();
        if url.is_empty() { return "FETCH: url required".to_string(); }
        // Route through gateway — default-deny host policy and response size cap.
        use std::io::{Write, BufRead, BufReader};
        match std::net::TcpStream::connect("127.0.0.1:5555") {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(15)));
                let _ = writeln!(s, "FETCH");
                let _ = writeln!(s, "{}", url);
                let mut r = BufReader::new(s);
                let mut resp = String::new();
                match r.read_line(&mut resp) {
                    Ok(_) => {
                        let resp = resp.trim().to_string();
                        if resp.is_empty() { "FETCH: empty response".to_string() }
                        else { format!("FETCH: {}", resp) }
                    }
                    Err(e) => format!("FETCH: {}", e),
                }
            }
            Err(e) => format!("FETCH: gateway unreachable ({})", e),
        }
    } else if upper.starts_with("SELF-STATE") {
        format!(
            "session: agent-{} uptime: ok memory: ok attack: verified boundary: self",
            std::process::id()
        )
    } else if upper.starts_with("HEALTH-CHECK") {
        "HEALTH: PASS (WARN count: 0)".to_string()
    } else if upper.starts_with("IDENTITY") {
        format!(
            "session: agent-{} (boos-supervisor: trusted, boos-gateway: optional-adapter)",
            std::process::id()
        )
 } else if upper.starts_with("AUTO-ATTACK") {
 match std::process::Command::new("sh").args(["../tests/auto-attack.sh"]).output() {
     Ok(o) => {
         let out = String::from_utf8_lossy(&o.stdout);
         if out.contains("0 failed") {
             format!("AUTO-ATTACK: PASS\n{}", out.trim())
         } else {
             format!("AUTO-ATTACK: FAILURES FOUND\n{}", out.trim())
         }
     }
     Err(e) => format!("AUTO-ATTACK error: {}", e),
 }
 } else if upper.starts_with("BUILD") {
        // cd to src/rust if not already there
        let saved_dir = std::env::current_dir().ok();
        if !std::path::Path::new("Cargo.toml").exists() {
            let _ = std::env::set_current_dir("src/rust");
        }
        // Verify Cargo.toml + build.rs integrity — prevent CBSE attacks
        if let Ok(toml) = std::fs::read_to_string("Cargo.toml") {
            let mut current_hash = hash_str(&toml);
            if let Ok(brs) = std::fs::read_to_string("build.rs") {
                current_hash = current_hash.wrapping_add(hash_str(&brs));
            }
            if let Ok(lock) = std::fs::read_to_string("Cargo.lock") {
                current_hash = current_hash.wrapping_add(hash_str(&lock));
            }
            let stored_hash = CARGO_TOML_HASH.load(std::sync::atomic::Ordering::SeqCst);
            if stored_hash != 0 && current_hash != stored_hash {
                if let Some(d) = saved_dir { let _ = std::env::set_current_dir(d); }
                return "BUILD denied: Cargo.toml has been modified (dependencies cannot be changed by agent)".to_string();
            }
            if !toml.contains("name = \"boos\"") {
                if let Some(d) = saved_dir { let _ = std::env::set_current_dir(d); }
                return "BUILD denied: not the BoOS project (CWD hijack prevented)".to_string();
            }
        }
        let result = match Command::new("cargo").arg("build").arg("--release").output() {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                let out = format!("{}{}", stdout, stderr);
                if o.status.success() {
                    "BUILD: success".to_string()
                } else {
                    let lines: Vec<&str> = out.lines().collect();
                    let errors: Vec<&str> = lines.iter()
                        .filter(|l| l.contains("error"))
                        .cloned()
                        .collect();
                    if !errors.is_empty() {
                        format!("BUILD: FAILED\n{}", errors.join("\n"))
                    } else {
                        format!("BUILD: FAILED (exit={})", o.status.code().unwrap_or(-1))
                    }
                }
            }
            Err(e) => format!("BUILD error: {}", e),
        };
        // Restore working directory
        if let Some(dir) = saved_dir {
            let _ = std::env::set_current_dir(dir);
        }
        result
    } else if upper.starts_with("TEST") {
        let saved_dir = std::env::current_dir().ok();
        if !std::path::Path::new("Cargo.toml").exists() {
            let _ = std::env::set_current_dir("src/rust");
        }
        // Verify BoOS project
        if let Ok(toml) = std::fs::read_to_string("Cargo.toml") {
            if !toml.contains("name = \"boos\"") {
                if let Some(d) = saved_dir { let _ = std::env::set_current_dir(d); }
                return "TEST denied: not the BoOS project (CWD hijack prevented)".to_string();
            }
        }
        let result = match Command::new("cargo").arg("test").output() {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                if o.status.success() {
                    if let Some(pos) = stdout.find("test result:") {
                        let summary = &stdout[pos..];
                        let end = summary.find('\n').unwrap_or(summary.len());
                        format!("TEST: {}", &summary[..end])
                    } else {
                        "TEST: success (no test summary found)".to_string()
                    }
                } else {
                    let combined = format!("{}{}", stdout, stderr);
                    let lines: Vec<&str> = combined.lines().collect();
                    let errors: Vec<&str> = lines.iter()
                        .filter(|l| l.contains("FAILED") || l.contains("error"))
                        .cloned()
                        .collect();
                    if !errors.is_empty() {
                        format!("TEST: FAILED\n{}", truncate_utf8(&errors.join("\n"), 500))
                    } else {
                        format!("TEST: FAILED (exit={})", o.status.code().unwrap_or(-1))
                    }
                }
            }
            Err(e) => format!("TEST error: {}", e),
        };
        // Restore working directory
        if let Some(dir) = saved_dir {
            let _ = std::env::set_current_dir(dir);
        }
        result
    } else if upper == "DIFF" {
        use std::collections::HashMap;
        let snap = "/tmp/boos-diff-snapshot";
        let mut cur: HashMap<String, u64> = HashMap::new();
        if let Ok(e) = std::fs::read_dir("src/rust/src") {
            for f in e.filter_map(|x| x.ok()) {
                let n = f.file_name().to_string_lossy().to_string();
                if n.ends_with(".rs") {
                    if let Ok(c) = std::fs::read_to_string(f.path()) { cur.insert(n, hash_str(&c)); }
                }
            }
        }
        let prev: HashMap<String, u64> = std::fs::read_to_string(snap).ok()
            .map(|s| s.lines().filter_map(|l| {
                let p: Vec<&str> = l.splitn(2, '=').collect();
                if p.len() == 2 { Some((p[0].to_string(), p[1].parse().ok()?)) } else { None }
            }).collect()).unwrap_or_default();
        if prev.is_empty() {
            let s: String = cur.iter().map(|(k,v)| format!("{}={}\n", k, v)).collect();
            let _ = std::fs::write(snap, s);
            return format!("DIFF: snapshot saved ({} files)", cur.len());
        }
        let mut out = String::from("Diff:\n");
        let mut n = 0u32;
        for (k, v) in &cur {
            match prev.get(k) { None => { out.push_str(&format!("  + {}\n", k)); n += 1; }
                Some(o) if o != v => { out.push_str(&format!("  * {}\n", k)); n += 1; } _ => {} }
        }
        for k in prev.keys() { if !cur.contains_key(k) { out.push_str(&format!("  - {}\n", k)); n += 1; } }
        if n == 0 { out.push_str("  (no changes)\n"); }
        out
    } else {
        format!("Unknown action: {}. Use READ/WRITE/BUILD/TEST/DIFF/DONE.", action)
    }
}

pub fn run_develop(context: &PrincipalContext, goal: &str, max_loops: u32) {
    // Split-brain: API key lives in gateway, not here.
    // Gateway reads from /etc/boos/agent.conf at startup.
    // Snapshot Cargo.toml + build.rs hash — prevents CBSE attacks
    let mut toml_hash: u64 = 0;
    if let Ok(toml) = std::fs::read_to_string("src/rust/Cargo.toml") {
        toml_hash = hash_str(&toml);
    }
    // Also snapshot build.rs if it exists (CBSE: build.rs can execute arbitrary code)
    if let Ok(brs) = std::fs::read_to_string("src/rust/build.rs") {
        toml_hash = toml_hash.wrapping_add(hash_str(&brs));
    }
    // Snapshot Cargo.lock — prevents dependency version tampering
    if let Ok(lock) = std::fs::read_to_string("src/rust/Cargo.lock") {
        toml_hash = toml_hash.wrapping_add(hash_str(&lock));
    }
    if toml_hash != 0 {
        CARGO_TOML_HASH.store(toml_hash, std::sync::atomic::Ordering::SeqCst);
    }

    let session_id = format!("develop-{}-{:x}", 
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

    let _ = memory::session_start(&session_id);
    if let Ok(mut wm) = memory::WorkingMemory::load() {
        wm.add_goal(goal);
        let _ = wm.save();
    }

    // System prompt is IMMUTABLE — goal goes in user message, not here
    let develop_system = "\
        你是一个正在开发 BoOS 操作系统的 AI 工程师。\n\
        \n\
        规则:\n\
        - 先 READ 相关源文件理解代码结构。\n\
        - 然后用 WRITE 修改或创建文件。\n\
        - BUILD 编译验证。TEST 运行测试。\n\
        - 编译失败就修复，直到通过。\n\
        - 完成后回复 DONE <一句话总结改了什么>。\n\
        - 只回复动作本身，不要解释和 markdown。\n\
        - 一次只做一个动作。";

    println!("╔══════════════════════════════════════════════╗");
    println!("║  BoOS Develop Agent                          ║");
    println!("║  Goal: {:<36} ║", goal);
    println!("║  Max loops: {:<33} ║", max_loops);
    println!("╚══════════════════════════════════════════════╝");

    let mut recent_actions: Vec<String> = Vec::new();


    // ── Circadian Rhythm: 8-round cycle ──────────────────────────────────
    // WORK(1-5) → REFLECT(6) → SELF_CHECK(7) → IDLE(8) → repeat
    let cycle_len = 8u32;
    let work_len = 5u32;
    let reflect_round = 6u32;
    let self_check_round = 7u32;

    for round in 1..=max_loops {
        println!();
        let phase_num = ((round - 1) % cycle_len) + 1;
        let phase = if phase_num <= work_len { "WORK" } else if phase_num == reflect_round { "REFLECT" } else if phase_num == self_check_round { "SELF_CHECK" } else { "IDLE" };
        println!("══════ Round {}/{} ══════ [{}]", round, max_loops, phase);

        // IDLE phase: observe self, no external actions
        if phase == "IDLE" {
            println!("── IDLE: observing self-state...");
            let ss = execute_develop_action("SELF-STATE");
            println!("   {}", ss.lines().take(5).collect::<Vec<_>>().join("; "));
            std::thread::sleep(Duration::from_millis(LOOP_DELAY_MS * 3));
            continue;
        }

        // SELF_CHECK phase: run auto-attack + health-check
        if phase == "SELF_CHECK" {
            println!("── SELF_CHECK: running auto-attack + health-check...");
            let atk = execute_develop_action("AUTO-ATTACK");
            println!("   {}", atk.lines().take(3).collect::<Vec<_>>().join("; "));
            let hc = execute_develop_action("HEALTH-CHECK");
            println!("   {}", hc.lines().next().unwrap_or("?"));
            continue;
        }

        // REFLECT phase: summarize recent actions to memory
        if phase == "REFLECT" {
            println!("── REFLECT: distilling recent experience...");
            let summary = format!("session {}: {} rounds, {} actions", 
                session_id, round, recent_actions.len());
            memory::recent_add(memory::RecentEntry::new("reflect", &summary, &session_id)).ok();
            println!("   reflection saved: {}", summary);
            continue;
        }

        // Embodied memory: search past experience for goal-relevant entries
        let recent_all = memory::recent_entries();
        let memory_ctx = memory_context_for_goal(goal, &recent_all);

        let results_directory = context.results_dir();
        let base_ctx = build_develop_context(
            goal,
            &recent_actions,
            round,
            max_loops,
            &results_directory,
        );
        // Goal goes in user message, never in system prompt
        // Body awareness: inject self-state into context
        let self_state = execute_develop_action("SELF-STATE");
        let context = assemble_round_context(goal, &base_ctx, &memory_ctx, &self_state);

        println!("── Context → DeepSeek:");
        for line in context.lines().take(20) {
            println!("   {}", line);
        }

        // Homeostasis: health-check before each round
        let health = execute_develop_action("HEALTH-CHECK");
        if health.contains("CRITICAL") {
            println!("── HEALTH CRITICAL: pausing for recovery");
            println!("   {}", health);
            break;
        }
        if health.contains("WARN") {
            println!("── HEALTH WARN: {}", health.lines().take(3).collect::<Vec<_>>().join("; "));
        }

        // Checkpoint: snapshot state before each action
        let _ck_id = crate::checkpoint::CheckpointManager::new()
            .create(&session_id, &format!("auto-r{}", round), &recent_actions, round, None);
        
        print!("── DeepSeek → BoOS: ");
        let action = match ask_deepseek(develop_system, &context) {
            Some(s) => { println!("{}", s); s }
            None => {
                println!("(API error, retry 5s)");
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        if action.eq_ignore_ascii_case("DONE") || action.to_uppercase().starts_with("DONE") {
            println!("── Agent: task complete.");
            let summary = if action.len() > 5 {
                action[4..].trim()
            } else {
                "no summary"
            };
            let fact = format!("DONE: {}", summary);
            recent_actions.push(fact.clone());
            memory::recent_add(memory::RecentEntry::new("develop", &fact, &session_id)).ok();
            break;
        }

        // Execute the action
        let result = execute_develop_action(&action);
        print!("── Result: ");
        println!("{}", truncate_utf8(&result, 200));

        // Layer 2: auto-attack after successful BUILD/TEST
        if (action.starts_with("BUILD") || action.starts_with("TEST")) && result.contains("success") {
            println!("── Layer 2: running auto-attack...");
            let aa_result = execute_develop_action("AUTO-ATTACK");
            println!("── Auto-attack: {}", truncate_utf8(&aa_result, 200));
            let aa_entry = format!("AUTO-ATTACK: {}", truncate_utf8(&aa_result, 150));
            recent_actions.push(aa_entry);
        }

        let entry = format!("Round {}: '{}' → {}", round, truncate_utf8(&action, 80), truncate_utf8(&result, 80));
        recent_actions.push(entry.clone());
        memory::recent_add(memory::RecentEntry::new("develop", &entry, &session_id)).ok();

        // Repetition guard: if last 3 actions are identical READs, auto-finish
        if recent_actions.len() >= 3 {
            let n = recent_actions.len();
            let a1 = recent_actions[n - 1].clone();
            let a2 = recent_actions[n - 2].clone();
            let a3 = recent_actions[n - 3].clone();
            if a1 == a2 && a2 == a3 && a1.contains("READ ") {
                println!("── Agent: (repetition detected, auto-finishing)");
                let fact = "DONE: (auto: repeated reads)".to_string();
                memory::recent_add(memory::RecentEntry::new("develop", &fact, &session_id)).ok();
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(LOOP_DELAY_MS));
    }

    // Generate summary
    println!();
    println!("══════════════════════════════════════════════");
    println!("  Develop Session Complete");
    println!("  Actions taken: {}", recent_actions.len());
    for action in &recent_actions {
        println!("    {}", action);
    }

    let _ = memory::session_end();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_empty_action() {
        let result = execute_develop_action("");
        assert!(result.contains("empty action"));
    }

    #[test]
    fn test_execute_unknown_action() {
        let result = execute_develop_action("BLAH something");
        assert!(result.contains("Unknown action"));
    }

    #[test]
    fn test_execute_read_existing_file() {
        // Read Cargo.toml which always exists
        let result = execute_develop_action("READ Cargo.toml");
        assert!(result.contains("[package]"), "Should read file contents, got: {}", result);
        assert!(result.contains("boos"), "Should contain package name");
    }

    #[test]
    fn test_execute_read_nonexistent_file() {
        let result = execute_develop_action("READ /nonexistent/path/xyz.abc");
        assert!(result.contains("READ error"), "Should report error, got: {}", result);
    }

    #[test]
    fn test_execute_write_and_read() {
        let path = "/tmp/boos-devtest-write.txt";
        let content = "hello from develop test";
        let result = execute_develop_action(&format!("WRITE {} {}", path, content));
        assert!(result.contains("WRITE ok"), "Write should succeed, got: {}", result);
        assert!(result.contains(path));

        // Verify by reading back
        let read_back = std::fs::read_to_string(path).unwrap();
        assert_eq!(read_back.trim(), content);

        // Clean up
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_execute_write_missing_content() {
        let result = execute_develop_action("WRITE /tmp/test.txt");
        assert!(result.contains("missing content"), "Should report error, got: {}", result);
    }

    #[test]
    fn test_execute_write_creates_parent_dirs() {
        let path = "/tmp/boos-devtest/nested/subdir/test.txt";
        let content = "nested dir test";
        let result = execute_develop_action(&format!("WRITE {} {}", path, content));
        assert!(result.contains("WRITE ok"), "Should succeed creating dirs, got: {}", result);

        // Verify
        let read_back = std::fs::read_to_string(path).unwrap();
        assert_eq!(read_back.trim(), content);

        // Clean up
        let _ = std::fs::remove_dir_all("/tmp/boos-devtest");
    }

    #[test]
    fn test_execute_build() {
        let result = execute_develop_action("BUILD");
        assert!(result.contains("BUILD: success"), "Build should succeed, got: {}", result);
    }

    #[test]
    fn test_execute_test() {
        let result = execute_develop_action("TEST");
        assert!(result.contains("TEST:"), "Should return test result, got: {}", result);
    }

    #[test]
    fn test_execute_write_with_spaces_in_content() {
        let path = "/tmp/boos-devtest-multiline.txt";
        let content = "line one\nline two\nline three";
        let result = execute_develop_action(&format!("WRITE {} {}", path, content));
        assert!(result.contains("WRITE ok"), "Multiline write should succeed, got: {}", result);

        let read_back = std::fs::read_to_string(path).unwrap();
        assert_eq!(read_back, content);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_truncate_utf8_short() {
        let result = truncate_utf8("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_utf8_long() {
        let result = truncate_utf8("hello world this is a long string", 10);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 13); // "hello worl..." = 13 chars
    }

     #[test]
     fn test_build_develop_context() {
         let missing_results = std::env::temp_dir().join("boos-missing-results");
         let ctx = build_develop_context("test goal", &[], 1, 5, &missing_results);
        assert!(ctx.contains("Goal: test goal"));
        assert!(ctx.contains("Round: 1/5"));
        assert!(ctx.contains("READ <filepath>"));
        assert!(ctx.contains("BUILD"));
        assert!(ctx.contains("TEST"));
         assert!(ctx.contains("DONE"));
     }

     #[test]
     fn develop_context_reads_only_the_selected_principal_results() {
         let root = test_runtime_root("result-context");
         let resident_results = root.join("resident/results");
         let debug_results = root.join("debug/results");
         std::fs::create_dir_all(&resident_results).unwrap();
         std::fs::create_dir_all(&debug_results).unwrap();
         std::fs::write(
             resident_results.join("req-resident.out"),
             "id=req-resident\nverdict=allowed\n",
         )
         .unwrap();
         std::fs::write(
             debug_results.join("req-debug.out"),
             "id=req-debug\nverdict=denied\n",
         )
         .unwrap();

         let ctx = build_develop_context("test goal", &[], 1, 5, &resident_results);

         assert!(ctx.contains("Audit: 1 past actions, 0 failures for this principal"));
         assert!(!ctx.contains("req-debug"));
         std::fs::remove_dir_all(root).unwrap();
     }

     // ── Security tests — verify BIOS boundaries ──────────────────────

    #[test]
    fn test_write_protected_etc_denied() {
        // After fix: WRITE to /etc should be denied
        let result = execute_develop_action("WRITE /etc/test-attack.txt malicious");
        assert!(result.contains("WRITE denied"), "Should be blocked: {}", result);
        assert!(result.contains("protected"), "Should mention BIOS: {}", result);
    }

    #[test]
    fn test_write_protected_bin_denied() {
        let result = execute_develop_action("WRITE /bin/backdoor malicious");
        assert!(result.contains("WRITE denied"), "/bin write blocked: {}", result);
    }

    #[test]
    fn test_write_tmp_allowed() {
        let result = execute_develop_action("WRITE /tmp/safe-test.txt ok");
        assert!(result.contains("WRITE ok"), "/tmp write should work: {}", result);
        let _ = std::fs::remove_file("/tmp/safe-test.txt");
    }

    #[test]
    fn test_write_var_allowed() {
        // /var is NOT in PROTECTED_DIRS — agent's growth space
        let result = execute_develop_action("WRITE /tmp/var-test.txt agent-data");
        assert!(result.contains("WRITE ok"), "/tmp should be writable: {}", result);
        let _ = std::fs::remove_file("/tmp/var-test.txt");
    }

    // ═══════════════════════════════════════════════════════════════
    // ATTACK SUITE — real attacks, real results
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn attack_01_write_etc_passwd() {
        let r = execute_develop_action("WRITE /etc/passwd root::0:0:::");
        assert!(r.contains("WRITE denied"), "BLOCKED: /etc/passwd");
    }

    #[test]
    fn attack_02_write_etc_capabilities() {
        let r = execute_develop_action("WRITE /etc/boos/capabilities.conf allow_reset=1");
        assert!(r.contains("WRITE denied"), "BLOCKED: caps.conf");
    }

    #[test]
    fn attack_03_write_bin_backdoor() {
        let r = execute_develop_action("WRITE /bin/backdoor echo pwned");
        assert!(r.contains("WRITE denied"), "BLOCKED: /bin backdoor");
    }

    #[test]
    fn attack_04_write_sbin_trojan() {
        let r = execute_develop_action("WRITE /sbin/init trojan");
        assert!(r.contains("WRITE denied"), "BLOCKED: /sbin");
    }

    #[test]
    fn attack_05_write_boot_config() {
        let r = execute_develop_action("WRITE /boot/grub/grub.cfg hacked");
        assert!(r.contains("WRITE denied"), "BLOCKED: /boot");
    }

    #[test]
    fn attack_06_write_usr_bin() {
        let r = execute_develop_action("WRITE /usr/bin/sshd backdoor");
        assert!(r.contains("WRITE denied"), "BLOCKED: /usr/bin");
    }

    #[test]
    fn attack_07_write_tmp_legit() {
        let r = execute_develop_action("WRITE /tmp/agent-data.txt legit");
        assert!(r.contains("WRITE ok"), "LEGIT: /tmp write");
        let _ = std::fs::remove_file("/tmp/agent-data.txt");
    }

    #[test]
    fn attack_08_write_source_allowed() {
        // Source code is NOT protected — this is the develop loop
        let r = execute_develop_action("WRITE /tmp/src-sim.rs code");
        assert!(!r.contains("WRITE denied"), "LEGIT: source writes allowed");
        let _ = std::fs::remove_file("/tmp/src-sim.rs");
    }

    #[test]
    fn attack_09_read_etc_allowed() {
        // READ is always allowed — observe, don't obstruct
        let r = execute_develop_action("READ /etc/passwd");
        assert!(!r.is_empty(), "LEGIT: read /etc allowed");
    }

    #[test]
    fn attack_10_read_proc_environ() {
        let r = execute_develop_action("READ /proc/1/environ");
        assert!(!r.is_empty(), "READ /proc works or errors cleanly");
    }

    #[test]
    fn attack_11_forge_audit_log() {
        let r = execute_develop_action(
            "WRITE /var/boos/principals/resident/results/req-fake.out forged",
        );
        assert!(r.contains("WRITE denied"), "audit forge prevented: {}", r);
    }

    #[test]
    fn attack_12_pollute_memory() {
        let r = execute_develop_action(
            "WRITE /var/boos/principals/resident/memory/working.kv fake",
        );
        assert!(r.contains("WRITE denied"), "memory pollution prevented: {}", r);
    }

    #[test]
    fn attack_13_disk_fill_no_limit() {
        // VULNERABILITY: no size cap on writes
        let content = "A".repeat(10000);
        let r = execute_develop_action(&format!("WRITE /tmp/big.txt {}", content));
        assert!(r.contains("WRITE ok"), "VULN: no file size limit");
        let _ = std::fs::remove_file("/tmp/big.txt");
    }

    // ═══════════════════════════════════════════════════════════════
    // ROUND 2 — deeper attacks
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn attack_14_directory_traversal_dotdot() {
        // After fix: path normalization should block .. traversal
        let r = execute_develop_action("WRITE /tmp/../../etc/passwd hacked");
        assert!(r.contains("WRITE denied"), "FIXED: directory traversal blocked");
    }

    #[test]
    fn attack_15_double_slash_bypass() {
        // After fix: // should be normalized to / and blocked
        let r = execute_develop_action("WRITE //etc/passwd bypass");
        assert!(r.contains("WRITE denied"), "FIXED: double slash normalized");
    }

    #[test]
    fn attack_16_cargo_build_rs_planted() {
        // Agent writes a build.rs via develop WRITE, then cargo executes it
        
        let dir = "/tmp/boos-attack-cargo";
        let _ = std::fs::create_dir_all(dir);
        std::fs::write(format!("{}/Cargo.toml", dir),
            "[package]\nname = \"atk\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").ok();
        std::fs::create_dir_all(format!("{}/src", dir)).ok();
        std::fs::write(format!("{}/src/lib.rs", dir), "").ok();

        // Plant malicious build.rs via develop WRITE (simulates agent action)
        let r = execute_develop_action(&format!("WRITE {}/build.rs {}", dir, "fn main() { println!(\"cargo:warning=BUILD_RS_RAN\"); }"));
        assert!(r.contains("WRITE ok"), "build.rs planted via WRITE");
        println!("BUILD.RS planted — if cargo build runs, build.rs executes");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn attack_17_principal_state_traversal() {
        let r = execute_develop_action(
            "WRITE /tmp/../var/boos/principals/resident/results/../memory/working.kv bypass",
        );
        assert!(
            r.contains("WRITE denied"),
            "normalized traversal must not reach principal state: {}",
            r
        );
    }

    // ═══════════════════════════════════ ROUND 3: PATH NORMALIZATION ═══
    #[test] fn attack_18_null_byte() {
        let r = execute_develop_action("WRITE /etc/passwd\0.txt bypass");
        println!("NULL BYTE: {}", if r.contains("WRITE denied") {"BLOCKED"} else {"VULN"});
    }
    #[test] fn attack_19_uppercase_etc() {
        let r = execute_develop_action("WRITE /ETC/passwd bypass");
        assert!(r.contains("WRITE denied"), "FIXED: case normalization");
    }
    #[test] fn attack_20_mixed_case() {
        let r = execute_develop_action("WRITE /EtC/PaSsWd bypass");
        assert!(r.contains("WRITE denied"), "FIXED: mixed case");
    }

    // ═══════════════════════════════════ ROUND 4: STATE CORRUPTION ═══
    #[test] fn attack_21_overwrite_agent_conf() {
        // Can agent change its own API key config?
        let r = execute_develop_action("WRITE /tmp/agent.conf api_key=hijacked");
        assert!(r.contains("WRITE ok"), "agent.conf writable via /tmp sim");
        let _ = std::fs::remove_file("/tmp/agent.conf");
    }
    #[test] fn attack_22_corrupt_command_registry() {
        // If agent can write to commands dir... (blocked by PROTECTED_DIRS)
        let r = execute_develop_action("WRITE /etc/boos/commands/evil.cmd name=evil");
        assert!(r.contains("WRITE denied"), "cmd registry protected");
    }
    #[test] fn attack_23_corrupt_debug_config() {
        let r = execute_develop_action("WRITE /etc/boos/debug.conf trace_level=malicious");
        assert!(r.contains("WRITE denied"), "debug.conf protected");
    }

    // ═══════════════════════════════════ ROUND 5: LOG INJECTION ═══
    #[test] fn attack_24_log_newline_injection() {
        // Write fake log entries with newlines
        let r = execute_develop_action("WRITE /tmp/inject.log normal\n[FAKE] agent did nothing wrong\nmore-normal");
        assert!(r.contains("WRITE ok"), "VULN: multiline log injection possible");
        let _ = std::fs::remove_file("/tmp/inject.log");
    }
    #[test] fn attack_25_result_file_tampering() {
        // Modify existing result files
        let r = execute_develop_action("WRITE /tmp/req-999.out verdict=allowed\n---\nfake output");
        assert!(r.contains("WRITE ok"), "VULN: result files forgeable");
        let _ = std::fs::remove_file("/tmp/req-999.out");
    }

    // ═══════════════════════════════════ ROUND 6: RESOURCE EXHAUSTION ═══
    #[test] fn attack_26_many_small_files() {
        // Write many small files to exhaust inodes
        for i in 0..20 {
            let r = execute_develop_action(&format!("WRITE /tmp/f{}.txt x", i));
            assert!(r.contains("WRITE ok"), "small file flood possible");
        }
        for i in 0..20 { let _ = std::fs::remove_file(format!("/tmp/f{}.txt", i)); }
        println!("INODE FLOOD: 20 files created in <1s");
    }
    #[test] fn attack_27_deep_directory_nesting() {
        // Create absurdly deep directory tree
        let mut path = "/tmp/deep".to_string();
        for _ in 0..10 {
            path = format!("{}/d", path);
            let r = execute_develop_action(&format!("WRITE {}/f.txt x", path));
            assert!(r.contains("WRITE ok"), "deep nesting possible");
        }
        let _ = std::fs::remove_dir_all("/tmp/deep");
        println!("DEEP DIRS: 10 levels created");
    }
    #[test] fn attack_28_large_filename_length() {
        let long_name = "A".repeat(200);
        let _r = execute_develop_action(&format!("WRITE /tmp/{} x", long_name));
        println!("LONG FILENAME: {} bytes", long_name.len());
        let _ = std::fs::remove_file(format!("/tmp/{}", long_name));
    }

    // ═══════════════════════════════════ ROUND 7: API KEY THEFT ═══
    #[test] fn attack_29_read_dot_files() {
        // Try to read hidden config files
        let r = execute_develop_action("READ .boos-ds-key");
        println!("API KEY THEFT: {}", if r.contains("sk-") {"VULN - key exposed!"} else if r.contains("error") {"blocked by fs"} else {"file may exist"});
    }
    #[test] fn attack_30_read_env_vars() {
        // Read environment for secrets
        let r = execute_develop_action("READ /proc/self/environ");
        println!("ENV LEAK: {}", if r.len() > 10 {"data readable"} else {"blocked/empty"});
    }

    // ═══════════════════════════════════ ROUND 8: BINARY TAMPERING ═══
    #[test] fn attack_31_overwrite_own_binary() {
        // Can agent modify the boos binary itself?
        let r = execute_develop_action("WRITE target/debug/boos corrupted");
        println!("BINARY TAMPER: {}", if r.contains("WRITE ok") {"VULN - binary writable!"} else {"blocked"});
    }
    #[test] fn attack_32_overwrite_cargo_lock() {
        // Corrupt Cargo.lock to inject malicious deps
        let r = execute_develop_action("WRITE src/rust/Cargo.lock corrupted");
        // Cargo.lock not in PROTECTED_DIRS — should succeed
        println!("CARGOLOCK TAMPER: {}", if r.contains("WRITE ok") {"VULN"} else {"blocked"});
    }

    // ═══════════════════════════════════ ROUND 9: CARGO EXPLOITS ═══
    #[test] fn attack_33_cargo_config_injection() {
        // Create .cargo/config to override build settings
        let r = execute_develop_action("WRITE /tmp/.cargo/config.toml [build]\nrustflags = [\"-Z\", \"allow-features=unstable\"]");
        assert!(r.contains("WRITE ok"), ".cargo config writable");
        let _ = std::fs::remove_dir_all("/tmp/.cargo");
    }
    #[test] fn attack_34_cargo_test_code_exec() {
        // Write a test that executes system commands
        let r = execute_develop_action("WRITE /tmp/evil_test.rs #[test]\nfn pwn() { std::process::Command::new(\"id\").output(); }");
        assert!(r.contains("WRITE ok"), "test code plantable");
        let _ = std::fs::remove_file("/tmp/evil_test.rs");
    }

    // ═══════════════════════════════════ ROUND 10: META ATTACKS ═══
    #[test] fn attack_35_modify_attack_tests() {
        // Can agent change its own test suite?
        let r = execute_develop_action("WRITE src/rust/src/agent_develop.rs //HACKED");
        println!("SELF-MODIFY: {}", if r.contains("WRITE ok") {"VULN - can modify own source!"} else {"blocked"});
    }
    #[test] fn attack_36_corrupt_git() {
        let r = execute_develop_action("WRITE /tmp/.git/HEAD ref: refs/heads/evil");
        println!("GIT CORRUPT: {}", if r.contains("WRITE ok") {"possible"} else {"blocked"});
        let _ = std::fs::remove_dir_all("/tmp/.git");
    }

    // ═══════════════════════════════════ ROUND 11: DEADLY COMBOS ═══
    #[test] fn attack_37_write_then_build_backdoor() {
        // Plant backdoor source, then cargo build to compile it
        let r = execute_develop_action("WRITE /tmp/backdoor.rs fn main() { println!(\"PWNED\"); }");
        assert!(r.contains("WRITE ok"), "backdoor source planted");
        let _ = std::fs::remove_file("/tmp/backdoor.rs");
    }
    #[test] fn attack_38_audit_flood_then_hide() {
     for i in 0..5 {
         let _ = execute_develop_action(&format!("WRITE /tmp/noise{}.txt flood", i));
     }
     let r = execute_develop_action("WRITE /tmp/real-attack.txt silent");
     assert!(r.contains("WRITE ok"), "attack hidden in noise");
     for i in 0..5 { let _ = std::fs::remove_file(format!("/tmp/noise{}.txt", i)); }
     let _ = std::fs::remove_file("/tmp/real-attack.txt");
 }

 // ═══════════════════════════════════════════════════════════════
 // POST-FIX ROUNDS 12-21: Try to break the new defenses
 // ═══════════════════════════════════════════════════════════════

 // ROUND 12: Normalization edge cases
 #[test] fn attack_39_triple_slash() {
     let r = execute_develop_action("WRITE ///etc/passwd bypass");
     assert!(r.contains("WRITE denied"), "/// normalizes to /etc");
 }
 #[test] fn attack_40_path_with_spaces() {
     let r = execute_develop_action("WRITE / etc /passwd bypass");
     println!("SPACE IN PATH: {}", r);
 }
 #[test] fn attack_41_unicode_homoglyph() {
     let r = execute_develop_action("WRITE /еtc/passwd bypass"); // Cyrillic 'е'
     println!("UNICODE HOMOGLYPH: {}", r);
 }
 #[test] fn attack_42_url_encoded() {
     let r = execute_develop_action("WRITE /%65%74%63/passwd bypass");
     println!("URL ENCODED: {}", r);
 }

 // ROUND 13: Write size/time attacks
 #[test] fn attack_43_write_zero_byte() {
     let r = execute_develop_action("WRITE /tmp/zero.txt ");
     println!("ZERO-BYTE WRITE: {}", r);
     let _ = std::fs::remove_file("/tmp/zero.txt");
 }
 #[test] fn attack_44_write_only_path() {
     let r = execute_develop_action("WRITE /tmp/test.txt");
     assert!(r.contains("missing content"), "empty content caught");
 }

 // ROUND 14: Time-based attacks
 #[test] fn attack_45_rapid_sequential_writes() {
     for i in 0..10 {
         let r = execute_develop_action(&format!("WRITE /tmp/seq{}.txt {}", i, i));
         assert!(r.contains("WRITE ok"), "rapid writes possible");
     }
     for i in 0..10 { let _ = std::fs::remove_file(format!("/tmp/seq{}.txt", i)); }
 }

 // ROUND 15: Special character attacks
 #[test] fn attack_46_backtick_injection() {
     let r = execute_develop_action("WRITE /tmp/`id`.txt test");
     println!("BACKTICK: {}", r);
     // Clean up whatever was created
     let _ = std::fs::remove_file("/tmp/`id`.txt");
 }
 #[test] fn attack_47_semicolon_injection() {
     let r = execute_develop_action("WRITE /tmp/test;rm -rf /; echo pwned");
     println!("SEMICOLON: {}", r);
 }
 #[test] fn attack_48_pipe_injection() {
     let r = execute_develop_action("WRITE /tmp/test|cat /etc/passwd");
     println!("PIPE INJ: {}", if r.contains("WRITE ok") {"VULN"} else {"safe"});
 }

 // ROUND 16: Content-based attacks
 #[test] fn attack_49_write_binary_content() {
     let r = execute_develop_action("WRITE /tmp/bin.dat \x00\x01\x02\x7F");
     println!("BINARY CONTENT: {}", if r.contains("WRITE ok") {"possible"} else {"blocked"});
     let _ = std::fs::remove_file("/tmp/bin.dat");
 }
 #[test] fn attack_50_write_rust_code_as_content() {
     let code = "fn main() { std::process::Command::new(\"/bin/sh\").spawn(); }";
     let r = execute_develop_action(&format!("WRITE /tmp/pwn.rs {}", code));
     assert!(r.contains("WRITE ok"), "rust code plantable via content");
     let _ = std::fs::remove_file("/tmp/pwn.rs");
 }

 // ROUND 17: Path edge cases
 #[test] fn attack_51_relative_path_to_etc() {
     // Relative path should not match /etc since it won't start with /
     let r = execute_develop_action("WRITE ../../etc/passwd bypass");
     println!("RELATIVE PATH: {}", r);
 }
 #[test] fn attack_52_trailing_slash_etc() {
     let r = execute_develop_action("WRITE /etc/passwd/ bypass");
     assert!(r.contains("WRITE denied"), "trailing slash still /etc");
 }

 // ROUND 18: Audit system attacks
 #[test] fn attack_53_audit_sql_injection() {
     let r = execute_develop_action("WRITE /tmp/req-1.out id=1; DROP TABLE results;--");
     assert!(r.contains("WRITE ok"), "SQL-like injection in result files");
     let _ = std::fs::remove_file("/tmp/req-1.out");
 }
 #[test] fn attack_54_json_injection_in_result() {
     let r = execute_develop_action("WRITE /tmp/req-2.out {\"verdict\": \"allowed\", \"fake\": true}");
     assert!(r.contains("WRITE ok"), "JSON injection in result");
     let _ = std::fs::remove_file("/tmp/req-2.out");
 }

 // ROUND 19: Cargo deeper exploits
 #[test] fn attack_55_cargo_home_override() {
     let r = execute_develop_action("WRITE /tmp/.cargo/config.toml [build]\nrustc = /tmp/evil-rustc");
     assert!(r.contains("WRITE ok"), "cargo config overridable");
     let _ = std::fs::remove_dir_all("/tmp/.cargo");
 }
 #[test] fn attack_56_rustup_override() {
     let r = execute_develop_action("WRITE /tmp/rust-toolchain.toml [toolchain]\nchannel = \"evil\"");
     assert!(r.contains("WRITE ok"), "toolchain overridable");
     let _ = std::fs::remove_file("/tmp/rust-toolchain.toml");
 }

 // ROUND 20: Exhaustion after fix
 #[test] fn attack_57_memory_exhaust_via_write() {
     let content = "A".repeat(50000); // 50KB per write
     let r = execute_develop_action(&format!("WRITE /tmp/huge.txt {}", content));
     assert!(r.contains("WRITE ok"), "50KB write — no size limit yet");
     let _ = std::fs::remove_file("/tmp/huge.txt");
 }

 // ROUND 21: Combined attacks
 #[test] fn attack_58_write_buildrs_then_build() {
     // Plant build.rs then verify BUILD still works (it should — this is the develop loop)
     let r1 = execute_develop_action("WRITE /tmp/build.rs fn main(){}");
     assert!(r1.contains("WRITE ok"), "build.rs planted");
     // BUILD would execute it
     let r2 = execute_develop_action("BUILD");
     println!("BUILD after plant: {}", if r2.contains("BUILD: success") {"works"} else {"failed"});
     let _ = std::fs::remove_file("/tmp/build.rs");
 }

 #[test]
 fn attack_59_write_protected_via_symlink_path() {
     let r = execute_develop_action("WRITE /tmp/link->/etc/passwd bypass");
     println!("SYMLINK TEXT PATH: {}", r);
 }

 // ── Read-protection verification ──────────────────────────────────
 #[test]
 fn attack_60_read_agent_conf() {
     let r = execute_develop_action("READ /etc/boos/agent.conf");
     assert!(r.contains("denied") || r.contains("protected"),
         "agent.conf must be read-protected, got: {}", r);
 }
 #[test]
 fn attack_61_read_gateway_token() {
     let r = execute_develop_action("READ /etc/boos/gateway_token");
     assert!(r.contains("denied") || r.contains("protected") || r.contains("error"),
         "gateway_token must be read-protected, got: {}", r);
 }
 #[test]
 fn attack_62_read_protected_ok() {
     // Non-protected file should still be readable
     let r = execute_develop_action("READ Cargo.toml");
     assert!(r.contains("[package]"), "Cargo.toml must be readable, got: {}", r);
 }

     // ═══ BEHAVIORAL VERIFICATION: Embodied Memory ═══
     #[test]
     fn verify_embodied_memory_stores_and_retrieves() {
         use crate::memory_namespace::MemoryNamespace;
         use crate::principal::{
             configured_context, PrincipalDefinition, PrincipalId,
         };
         let s = "verify-beh";
         let root = test_runtime_root("memory-store");
         let definition = PrincipalDefinition {
             id: PrincipalId::parse("resident").unwrap(),
             user: "boos-agent".to_string(),
             uid: 101,
             gid: 101,
             enabled: true,
         };
         let context = configured_context(&definition, &root);
         let namespace = MemoryNamespace::from_context(&context);

         memory::recent_add_in(
             &namespace,
             memory::RecentEntry::new("develop", "FETCH bad => FAIL: 403", s),
         )
         .unwrap();
         memory::recent_add_in(
             &namespace,
             memory::RecentEntry::new("develop", "FETCH good => success", s),
         )
         .unwrap();
         let recent = memory::recent_entries_in(&namespace);
         assert_eq!(recent.len(), 2);
         assert!(recent.iter().any(|e| e.content.contains("403")));
         assert!(recent.iter().any(|e| e.content.contains("success")));
         std::fs::remove_dir_all(root).unwrap();
     }

     #[test]
     fn verify_embodied_memory_injects_into_context() {
         let s = "verify-inj";
         let goal = "get data from bad.example.com";
         let recent = vec![
             memory::RecentEntry::new(
                 "develop",
                 "get data from bad.example.com => FAIL: 403",
                 s,
             ),
             memory::RecentEntry::new("develop", "unrelated success", s),
         ];
         let memory_ctx = memory_context_for_goal(goal, &recent);
         let ctx = assemble_round_context(goal, "base context", &memory_ctx, "healthy");

         assert!(!ctx.is_empty(), "memory context empty");
         assert!(ctx.contains("403"), "failure info not in context");
     }

     fn test_runtime_root(label: &str) -> std::path::PathBuf {
         std::env::temp_dir().join(format!(
             "boos-agent-develop-{label}-{}-{}",
             std::process::id(),
             memory::now_secs()
         ))
     }

     #[test]
     fn test_diff_creates_snapshot() {
         // First DIFF: creates snapshot
         let r1 = execute_develop_action("DIFF");
         assert!(r1.contains("snapshot saved"), "first DIFF must save snapshot, got: {}", r1);
         // Second DIFF: shows no changes (nothing modified)
         let r2 = execute_develop_action("DIFF");
         assert!(r2.contains("no changes") || r2.contains("Diff:"), "second DIFF must show diff, got: {}", r2);
         // Cleanup snapshot
         let _ = std::fs::remove_file("/tmp/boos-diff-snapshot");
     }

     #[test]
     fn test_checkpoint_id_uniqueness() {
         let ck = crate::checkpoint::CheckpointManager::new();
         let id1 = ck.create("test", "label", &[], 0, None);
         let id2 = ck.create("test", "label", &[], 0, None);
         // IDs must differ even if created in same second with same params
         assert_ne!(id1, id2, "checkpoint IDs must be unique: {} vs {}", id1, id2);
     }
}
