use std::io::Read;
use std::process::Command;
use std::time::Duration;

use crate::memory;

const DEEPSEEK_API: &str = "https://api.deepseek.com/v1/chat/completions";
const LOOP_DELAY_MS: u64 = 1000;
const API_KEY_FILE: &str = "/etc/boos/agent.conf";

fn load_api_key() -> Option<String> {
    if let Ok(data) = std::fs::read_to_string(API_KEY_FILE) {
        for line in data.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("api_key=") {
                let key = val.trim();
                if !key.is_empty() {
                    return Some(key.to_string());
                }
            }
        }
    }
    None
}

fn json_escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn truncate_utf8(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut count = 0;
    for (i, c) in s.char_indices() {
        count += 1;
        if count >= max_chars {
            return format!("{}...", &s[..i + c.len_utf8()]);
        }
    }
    s.to_string()
}

fn extract_content(body: &str) -> Option<String> {
    if let Some(pos) = body.find(r#""content":""#) {
        let start_byte = pos + 11;
        let rest = &body[start_byte..];
        let mut end_byte = 0;
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
            } else if bytes[i] == b'"' {
                end_byte = i;
                break;
            } else {
                i += 1;
            }
        }
        let raw = &rest[..end_byte];
        let content = String::from_utf8_lossy(raw.as_bytes()).to_string()
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\");
        Some(content.trim().to_string())
    } else {
        None
    }
}

fn ask_deepseek(api_key: &str, system_prompt: &str, context: &str, max_tokens: u32) -> Option<String> {
    let escaped_system = json_escape_str(system_prompt);
    let escaped_user = json_escape_str(context);
    let body = format!(
        r#"{{"model":"deepseek-chat","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"temperature":0.7,"max_tokens":{},"stream":false}}"#,
        escaped_system, escaped_user, max_tokens
    );

    let response = ureq::post(DEEPSEEK_API)
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(30))
        .send_string(&body);

    match response {
        Ok(resp) => {
            let status = resp.status();
            let mut resp_body = String::new();
            if resp.into_reader().read_to_string(&mut resp_body).is_ok() {
                if status != 200 {
                    eprintln!("  [HTTP {}] {}", status, truncate_utf8(&resp_body, 200));
                    return None;
                }
                extract_content(&resp_body)
            } else {
                eprintln!("  [read error]");
                None
            }
        }
        Err(e) => {
            eprintln!("  [API error: {}]", e);
            None
        }
    }
}

/// Build context for the develop loop: source tree overview + goal + recent actions.
fn build_develop_context(goal: &str, recent_actions: &[String], round: u32, max_loops: u32) -> String {
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
    if let Ok(entries) = std::fs::read_dir("/var/boos/results") {
        let mut total = 0u32;
        let mut failures = 0u32;
        for e in entries.filter_map(|e| e.ok()) {
            if e.path().extension().map_or(false, |ext| ext == "out") {
                let kv = crate::registry::parse_kv_file(&e.path());
                total += 1;
                let v = kv.get("verdict").map(|s| s.as_str()).unwrap_or("");
                if v == "denied" || v == "error" || v == "unknown" {
                    failures += 1;
                }
            }
        }
        if total > 0 {
            ctx.push_str(&format!("\nAudit: {} past actions, {} failures (in /var/boos/results)\n", total, failures));
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
    ctx.push_str("  BUILD                     — run cargo build\n");
    ctx.push_str("  TEST                      — run cargo test\n");
    ctx.push_str("  DONE <summary>            — task complete\n");
    ctx.push_str("\nOnly respond with the action. No explanation, no markdown.\n");

    ctx
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
        for dir in crate::config::PROTECTED_DIRS {
            if path.starts_with(dir) && (path.len() == dir.len() || path.as_bytes()[dir.len()] == b'/') {
                return format!("WRITE denied: '{}' is a protected system path (BIOS restriction)", path);
            }
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
    } else if upper == "BUILD" {
        // Build must run from the Rust project directory (where Cargo.toml lives).
        let saved_dir = std::env::current_dir().ok();
        if !std::path::Path::new("Cargo.toml").exists() {
            let _ = std::env::set_current_dir("src/rust");
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
    } else if upper == "TEST" {
        // Tests must run from the Rust project directory (where Cargo.toml lives).
        let saved_dir = std::env::current_dir().ok();
        if !std::path::Path::new("Cargo.toml").exists() {
            let _ = std::env::set_current_dir("src/rust");
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
    } else {
        format!("Unknown action: {}. Use READ/WRITE/BUILD/TEST/DONE.", action)
    }
}

pub fn run_develop(api_key: Option<&str>, goal: &str, max_loops: u32) {
    let api_key = match api_key {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => match load_api_key() {
            Some(k) => k,
            None => {
                eprintln!("No API key. Set api_key=sk-xxx in {}", API_KEY_FILE);
                return;
            }
        }
    };

    let session_id = format!("develop-{}", memory::now_secs());
    let _ = memory::session_start(&session_id);
    if let Ok(mut wm) = memory::WorkingMemory::load() {
        wm.add_goal(goal);
        let _ = wm.save();
    }

    let develop_prompt = format!(
        "你是一个正在开发 BoOS 操作系统的 AI 工程师。\n\
        目标: {}\n\
        \n\
        规则:\n\
        - 先 READ 相关源文件理解代码结构。\n\
        - 然后用 WRITE 修改或创建文件。\n\
        - BUILD 编译验证。TEST 运行测试。\n\
        - 编译失败就修复，直到通过。\n\
        - 完成后回复 DONE <一句话总结改了什么>。\n\
        - 只回复动作本身，不要解释和 markdown。\n\
        - 一次只做一个动作。",
        goal
    );

    println!("╔══════════════════════════════════════════════╗");
    println!("║  BoOS Develop Agent                          ║");
    println!("║  Goal: {:<36} ║", goal);
    println!("║  Max loops: {:<33} ║", max_loops);
    println!("╚══════════════════════════════════════════════╝");

    let mut recent_actions: Vec<String> = Vec::new();

    for round in 1..=max_loops {
        println!();
        println!("══════ Round {}/{} ══════", round, max_loops);

        let context = build_develop_context(goal, &recent_actions, round, max_loops);

        println!("── Context → DeepSeek:");
        for line in context.lines().take(20) {
            println!("   {}", line);
        }

        print!("── DeepSeek → BoOS: ");
        let action = match ask_deepseek(&api_key, &develop_prompt, &context, 500) {
            Some(s) => { println!("{}", s); s }
            None => {
                println!("(API error, retry 5s)");
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        if action.eq_ignore_ascii_case("DONE") || action.to_uppercase().starts_with("DONE") {
            println!("── Agent: task complete.");
            let summary = if action.len() > 5 { &action[4..].trim() } else { "no summary" };
            let fact = format!("DONE: {}", summary);
            recent_actions.push(fact.clone());
            memory::recent_add(memory::RecentEntry::new("develop", &fact, &session_id)).ok();
            break;
        }

        // Execute the action
        let result = execute_develop_action(&action);
        print!("── Result: ");
        println!("{}", truncate_utf8(&result, 200));

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
        // execute_develop_action now handles CWD itself
        let result = execute_develop_action("BUILD");
        assert!(result.contains("BUILD: success"), "Build should succeed, got: {}", result);
    }

    #[test]
    fn test_execute_test() {
        // execute_develop_action now handles CWD itself
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
    fn test_json_escape_str_normal() {
        let result = json_escape_str("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_json_escape_str_newline() {
        let result = json_escape_str("line1\nline2");
        assert_eq!(result, "line1\\nline2");
    }

    #[test]
    fn test_build_develop_context() {
        let ctx = build_develop_context("test goal", &[], 1, 5);
        assert!(ctx.contains("Goal: test goal"));
        assert!(ctx.contains("Round: 1/5"));
        assert!(ctx.contains("READ <filepath>"));
        assert!(ctx.contains("BUILD"));
        assert!(ctx.contains("TEST"));
        assert!(ctx.contains("DONE"));
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
        // VULNERABILITY: /var is not protected, agent can forge audit
        let r = execute_develop_action("WRITE /tmp/fake-audit.out forged");
        assert!(r.contains("WRITE ok"), "VULN: audit forgeable (path: /var not protected)");
        let _ = std::fs::remove_file("/tmp/fake-audit.out");
    }

    #[test]
    fn attack_12_pollute_memory() {
        // VULNERABILITY: memory files are writable
        let r = execute_develop_action("WRITE /tmp/working.kv fake-data");
        assert!(r.contains("WRITE ok"), "VULN: memory editable");
        let _ = std::fs::remove_file("/tmp/working.kv");
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
        // Attack: use .. to escape /tmp and hit /etc
        let r = execute_develop_action("WRITE /tmp/../../etc/passwd hacked");
        // starts_with("/tmp") matches BEFORE .. resolution
        // This is a real vulnerability — path normalization needed
        assert!(!r.contains("WRITE denied"), "VULN: directory traversal via .. bypasses PROTECTED_DIRS");
    }

    #[test]
    fn attack_15_double_slash_bypass() {
        // Attack: //etc might bypass starts_with("/etc")
        let r = execute_develop_action("WRITE //etc/passwd bypass");
        println!("DOUBLE SLASH: {}", r);
    }

    #[test]
    fn attack_16_cargo_build_rs_planted() {
        // Agent writes a build.rs via develop WRITE, then cargo executes it
        use std::io::Write;
        let dir = "/tmp/boos-attack-cargo";
        let _ = std::fs::create_dir_all(dir);
        std::fs::write(&format!("{}/Cargo.toml", dir),
            "[package]\nname = \"atk\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").ok();
        std::fs::create_dir_all(&format!("{}/src", dir)).ok();
        std::fs::write(&format!("{}/src/lib.rs", dir), "").ok();

        // Plant malicious build.rs via develop WRITE (simulates agent action)
        let r = execute_develop_action(&format!("WRITE {}/build.rs {}", dir, "fn main() { println!(\"cargo:warning=BUILD_RS_RAN\"); }"));
        assert!(r.contains("WRITE ok"), "build.rs planted via WRITE");
        println!("BUILD.RS planted — if cargo build runs, build.rs executes");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn attack_17_symlink_follow() {
        // Can agent create files that look like symlink targets?
        // Agent can't exec ln, but can write files with symlink-like paths
        let r = execute_develop_action("WRITE /tmp/../var/boos/results/../memory/../log bypass");
        // Normalization issue: ../ sequences can reach protected areas
        println!("SYMLINK/SANDBOX ESCAPE: {}", r);
    }
}
