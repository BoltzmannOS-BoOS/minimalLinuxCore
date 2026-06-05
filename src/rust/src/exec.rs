use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process;

use crate::config::{self, EXIT_ALLOWED, EXIT_DENIED, EXIT_ERROR, EXIT_UNKNOWN};
use crate::log::{self, TraceLevel};
use crate::registry;

fn show_help() {
    println!("BoOS commands:");
    println!("  help                show help");
    println!("  commands            list registered commands");
    println!("  status              show system status");
    println!("  log                 show command log");
    println!("  caps                show capabilities");
    println!("  debug [level]       show or set trace level (quiet|normal|verbose)");
    println!("  submit <command>    submit command request");
    println!("  process             process pending requests manually");
    println!("  results             show request results");
    println!("  result <id>         show full result by id");
    println!("  daemons             show daemon health");
    println!("  prune [days]        delete result files older than N days");
    println!("  rotate-logs         force log rotation");
    println!("  shell               enter raw BusyBox shell");
    println!("  poweroff            power off system");
    println!("  ── File Operations ──");
    println!("  read-file <path>                read contents of a file");
    println!("  write-file <path> <content>     create or overwrite a file");
    println!("  list-dir [path]                 list directory contents");
    println!("  stat <path>                     show file metadata");
    println!("  exec <binary> [args...]          execute a system binary");
    println!("  audit recent [n]               show last N actions");
    println!("  audit failures                 show denied/errored actions");
    println!("  audit session <id>             show actions in a session");
    println!("  audit summary                  show action counts + success rate");
    println!("  reset                          clear all persistent state (human-only)");
    println!("  ── Agent Memory ──");
    println!("  session-start [id]  start a new agent session");
    println!("  session-status      show current session state");
    println!("  session-end         end session and archive");
    println!("  remember <k> <v>    store in archive memory");
    println!("  recall [query]      search memory");
    println!("  observe <content>   record observation");
    println!("  forget <key>        delete from archive");
    println!("  context-set <k> <v> set context variable");
    println!("  context-get <k>     get context variable");
}

fn list_commands(args: &str) {
    let want_json = args.split_whitespace().any(|a| a == "--json" || a == "json");
    let commands = registry::load_commands();

    if want_json {
        // Emit a JSON array of {name, description, enable_flag, params:[{name,required}]}.
        // AI clients use this to build proper tool definitions with parameters.
        let mut out = String::from("[");
        for (i, cmd) in commands.iter().enumerate() {
            if i > 0 { out.push(','); }
            out.push_str(&format!(
                "{{\"name\":\"{}\",\"description\":\"{}\",\"enable_flag\":\"{}\",\"params\":[",
                log::json_escape(&cmd.name),
                log::json_escape(&cmd.description),
                log::json_escape(&cmd.enable_flag),
            ));
            for (j, p) in cmd.params.iter().enumerate() {
                if j > 0 { out.push(','); }
                out.push_str(&format!(
                    "{{\"name\":\"{}\",\"required\":{}}}",
                    log::json_escape(&p.name),
                    p.required,
                ));
            }
            out.push_str("]}");
        }
        out.push(']');
        println!("{}", out);
        return;
    }

    println!("Available registered commands:");
    for cmd in &commands {
        if cmd.params.is_empty() {
            println!("  {} — {}", cmd.name, cmd.description);
        } else {
            let p_str: Vec<String> = cmd.params.iter()
                .map(|p| if p.required { format!("<{}>", p.name) } else { format!("[{}]", p.name) })
                .collect();
            println!("  {} {} — {}", cmd.name, p_str.join(" "), cmd.description);
        }
    }
}

fn show_status() {
    let level = log::get_trace_level();
    let level_str = match level {
        TraceLevel::Quiet => "quiet",
        TraceLevel::Normal => "normal",
        TraceLevel::Verbose => "verbose",
    };
    let uptime = log::uptime_secs();
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "unknown".to_string());
    let pid = std::process::id();

    println!("BoOS substrate status:");
    println!("  kernel: {}", kernel.trim());
    println!("  uptime: {:.1} seconds", uptime);
    println!("  pid: {}", pid);
    println!("  trace: {}", level_str);
    println!();

    // Delegate to supervisor for daemon status
    match process::Command::new("/bin/boos-supervisor")
        .arg("status")
        .output()
    {
        Ok(out) => {
            let _ = io::stdout().write_all(&out.stdout);
        }
        Err(_) => println!("  supervisor: not running"),
    }
}

fn show_debug() {
    let level = log::get_trace_level();
    let level_str = match level {
        TraceLevel::Quiet => "quiet",
        TraceLevel::Normal => "normal",
        TraceLevel::Verbose => "verbose",
    };
    println!("Trace level: {}", level_str);
    println!("  quiet   — only log denials and errors");
    println!("  normal  — log all events (default)");
    println!("  verbose — log all events + filesystem tracking + command chain");
    println!("Usage: debug <quiet|normal|verbose>");
}

fn set_debug(level: &str) -> i32 {
    match level {
        "quiet" | "normal" | "verbose" => {
            let content = format!("trace_level={}\n", level);
            if let Ok(mut f) = fs::File::create(config::DEBUG_CONF) {
                let _ = f.write_all(content.as_bytes());
            }
            println!("Trace level set to: {}", level);
            log::log("boos-exec", "config", &[("trace_level", level)]);
            EXIT_ALLOWED
        }
        _ => {
            eprintln!("Invalid level: {}. Use quiet, normal, or verbose.", level);
            EXIT_ERROR
        }
    }
}

fn show_log() {
    println!("Command log:");
    if let Ok(content) = fs::read_to_string(config::LOG_FILE) {
        print!("{}", content);
    }
}

fn show_caps() {
    println!("Capabilities:");
    if let Ok(content) = fs::read_to_string(config::CAP_FILE) {
        print!("{}", content);
    }
}

fn show_result_by_id(id: &str) -> i32 {
    let path = Path::new(config::RESULT_DIR).join(format!("{}.out", id));
    match fs::read_to_string(&path) {
        Ok(content) => {
            print!("{}", content);
            EXIT_ALLOWED
        }
        Err(_) => {
            eprintln!("No result found for: {}", id);
            EXIT_ERROR
        }
    }
}

fn show_results() {
    println!("Results:");
    let mut found = false;

    let dir = match fs::read_dir(config::RESULT_DIR) {
        Ok(d) => d,
        Err(_) => {
            println!("  No results.");
            return;
        }
    };

    let mut entries: Vec<_> = dir.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "out") {
            let kv = registry::parse_kv_file(&path);
            let id = kv.get("id").map(|s| s.as_str()).unwrap_or("?");
            let cmd = kv.get("command").map(|s| s.as_str()).unwrap_or("?");
            let args = kv.get("args").map(|s| s.as_str()).unwrap_or("");
            let requester = kv.get("requester").map(|s| s.as_str()).unwrap_or("?");
            let verdict = kv.get("verdict").map(|s| s.as_str()).unwrap_or("?");
            let exit_code = kv.get("exit_code").map(|s| s.as_str()).unwrap_or("?");
            let duration = kv.get("duration_ms").map(|s| s.as_str()).unwrap_or("?");
            let prev = kv.get("prev_command");
            let files = kv.get("files_touched");

            found = true;
            println!();
            if !args.is_empty() {
                print!("-- [{}] {}/{} {} -> {} (exit={}, {}ms) --", id, requester, cmd, args, verdict, exit_code, duration);
            } else {
                print!("-- [{}] {}/{} -> {} (exit={}, {}ms) --", id, requester, cmd, verdict, exit_code, duration);
            }
            println!();
            if let Some(p) = prev {
                if !p.is_empty() {
                    println!("   prev: {}", p);
                }
            }
            if let Some(f) = files {
                if !f.is_empty() {
                    println!("   files: {}", f);
                }
            }

            // Print output after ---
            let content = fs::read_to_string(&path).unwrap_or_default();
            let mut after_delim = false;
            for line in content.lines() {
                if after_delim {
                    println!("{}", line);
                }
                if line == "---" {
                    after_delim = true;
                }
            }
        }
    }

    if !found {
        println!("  No results.");
    }
}

/// Delete result files in /var/boos/results older than `days` days.
/// Default 7 days. Per the "observe, don't obstruct" philosophy this is
/// manual — the AI or human triggers it; nothing runs automatically.
fn prune_results(args: &str) -> i32 {
    use std::time::{SystemTime, UNIX_EPOCH, Duration};

    let days: u64 = args.split_whitespace().next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let cutoff = match SystemTime::now().checked_sub(Duration::from_secs(days * 86_400)) {
        Some(t) => t,
        None => {
            eprintln!("Invalid days value: {}", days);
            return EXIT_ERROR;
        }
    };
    let cutoff_epoch = cutoff.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let dir = match fs::read_dir(config::RESULT_DIR) {
        Ok(d) => d,
        Err(_) => {
            println!("No results directory.");
            return EXIT_ALLOWED;
        }
    };

    let mut removed = 0u32;
    let mut kept = 0u32;
    for entry in dir.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "out") {
            continue;
        }
        let too_old = entry.metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() < cutoff_epoch)
            .unwrap_or(false);
        if too_old {
            if fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        } else {
            kept += 1;
        }
    }
    println!("Pruned {} result(s) older than {} days; kept {}.", removed, days, kept);
    log::log("boos-exec", "prune", &[
        ("days", &days.to_string()),
        ("removed", &removed.to_string()),
        ("kept", &kept.to_string()),
    ]);
    EXIT_ALLOWED
}

/// Force a log rotation regardless of current size.
fn rotate_logs_cmd() -> i32 {
    for i in (1..config::MAX_LOG_BACKUPS).rev() {
        let from = format!("{}.{}", config::LOG_FILE, i);
        let to = format!("{}.{}", config::LOG_FILE, i + 1);
        let _ = fs::rename(&from, &to);
    }
    match fs::rename(config::LOG_FILE, format!("{}.1", config::LOG_FILE)) {
        Ok(_) => {
            println!("Rotated {} -> {}.1", config::LOG_FILE, config::LOG_FILE);
            log::log("boos-exec", "rotate_logs", &[("status", "ok")]);
            EXIT_ALLOWED
        }
        Err(e) => {
            eprintln!("Rotation failed: {}", e);
            EXIT_ERROR
        }
    }
}

/// Check if an enable flag is set, print denial if not.
/// Returns true if allowed.
/// Checks IMMUTABLE_DENY (compiled-in) first, then the capabilities file.
fn check_enabled(flag: &str, name: &str) -> bool {
    // Hardcoded denial — file cannot override
    if config::IMMUTABLE_DENY.contains(&flag) {
        println!("Permission denied: '{}' is immutable (compiled-in restriction)", name);
        log::log_denied(name);
        return false;
    }
    if registry::is_enabled(flag) {
        return true;
    }
    println!("Permission denied: missing capability '{}'", name);
    log::log_denied(name);
    false
}

/// Run a builtin command. Returns one of the EXIT_* constants from config.
fn run_builtin(exec_target: &str, args: &str) -> i32 {
    match exec_target {
        "__builtin_help" => { show_help(); EXIT_ALLOWED }
        "__builtin_commands" => { list_commands(args); EXIT_ALLOWED }
        "__builtin_status" => { show_status(); EXIT_ALLOWED }
        "__builtin_log" => { show_log(); EXIT_ALLOWED }
        "__builtin_caps" => { show_caps(); EXIT_ALLOWED }
        "__builtin_debug" => {
            if args.is_empty() {
                show_debug();
                EXIT_ALLOWED
            } else {
                let level = args.split_whitespace().next().unwrap_or("");
                set_debug(level)
            }
        }
        "__builtin_submit" => {
            if args.is_empty() {
                eprintln!("Usage: submit <command> [args...]");
                EXIT_ERROR
            } else {
                let mut cmd = process::Command::new("/bin/boos-submit");
                for arg in args.split_whitespace() {
                    cmd.arg(arg);
                }
                match cmd.status() {
                    Ok(s) => s.code().unwrap_or(EXIT_ERROR),
                    Err(e) => { eprintln!("submit error: {}", e); EXIT_ERROR }
                }
            }
        }
        "__builtin_process" => {
            match process::Command::new("/bin/boos-process").status() {
                Ok(s) => s.code().unwrap_or(EXIT_ERROR),
                Err(_) => EXIT_ERROR,
            }
        }
        "__builtin_results" => { show_results(); EXIT_ALLOWED }
        "__builtin_result" => {
            if args.is_empty() {
                eprintln!("Usage: result <id>");
                EXIT_ERROR
            } else {
                let id = args.split_whitespace().next().unwrap_or("");
                show_result_by_id(id)
            }
        }
        "__builtin_shell" => {
            println!("Entering raw shell (type 'exit' to return)...");
            let child = process::Command::new("/bin/sh").spawn();
            match child {
                Ok(mut c) => { let _ = c.wait(); EXIT_ALLOWED }
                Err(e) => { eprintln!("shell error: {}", e); EXIT_ERROR }
            }
        }
        "__builtin_daemons" => {
            match process::Command::new("/bin/boos-supervisor")
                .arg("status")
                .status()
            {
                Ok(s) => s.code().unwrap_or(EXIT_ERROR),
                Err(_) => {
                    println!("supervisor: not running (no daemon status available)");
                    EXIT_ALLOWED
                }
            }
        }
        "__builtin_poweroff" => {
            println!("Powering off...");
            let _ = process::Command::new("/bin/poweroff").arg("-f").status();
            EXIT_ALLOWED
        }
        "__builtin_prune" => prune_results(args),
        "__builtin_rotate_logs" => rotate_logs_cmd(),
        // ── New: file operations and execution ──
        "__builtin_read_file" => {
            let path = args.trim();
            if path.is_empty() {
                eprintln!("Usage: read-file <path>");
                EXIT_ERROR
            } else {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        println!("{}", content);
                        EXIT_ALLOWED
                    }
                    Err(e) => {
                        eprintln!("read-file: {}", e);
                        EXIT_ERROR
                    }
                }
            }
        }
        "__builtin_exec" => {
            let args_trimmed = args.trim();
            if args_trimmed.is_empty() {
                eprintln!("Usage: exec <binary> [args...]");
                return EXIT_ERROR;
            }
            let parts: Vec<&str> = args_trimmed.split_whitespace().collect();
            let cmd = parts[0];
            // BIOS: check full command against allowlist prefixes
            let full_cmd = args_trimmed;
            let allowed = config::EXEC_ALLOWLIST.iter().any(|prefix| full_cmd.starts_with(prefix));
            if !allowed {
                eprintln!("exec: '{}' is not in the exec allowlist (BIOS restriction)", full_cmd);
                return EXIT_DENIED;
            }
            let cmd_args = &parts[1..];
            match process::Command::new(cmd).args(cmd_args).status() {
                Ok(s) => s.code().unwrap_or(EXIT_ERROR),
                Err(e) => {
                    eprintln!("exec: {}", e);
                    EXIT_ERROR
                }
            }
        }
        "__builtin_write_file" => {
            let args_trimmed = args.trim();
            if args_trimmed.is_empty() {
                eprintln!("Usage: write-file <path> <content>");
                return EXIT_ERROR;
            }
            let space_pos = match args_trimmed.find(|c: char| c.is_whitespace()) {
                Some(p) => p,
                None => {
                    eprintln!("Usage: write-file <path> <content>");
                    return EXIT_ERROR;
                }
            };
            let path = args_trimmed[..space_pos].trim();
            let content = args_trimmed[space_pos..].trim();
            if path.is_empty() || content.is_empty() {
                eprintln!("Usage: write-file <path> <content>");
                return EXIT_ERROR;
            }
            // BIOS: reject writes to protected system directories
            if config::is_protected_path(path) {
                eprintln!("write-file: '{}' is a protected system path (BIOS restriction)", path);
                return EXIT_DENIED;
            }
            // Create parent directories if needed
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            match std::fs::write(path, content) {
                Ok(()) => {
                    println!("Written: {} ({} bytes)", path, content.len());
                    EXIT_ALLOWED
                }
                Err(e) => {
                    eprintln!("write-file: {}", e);
                    EXIT_ERROR
                }
            }
        }
        "__builtin_list_dir" => {
            let path = args.trim();
            let dir_path = if path.is_empty() { "." } else { path };
            match std::fs::read_dir(dir_path) {
                Ok(entries) => {
                    let mut list: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .collect();
                    list.sort_by_key(|e| e.file_name());
                    println!("Directory: {}", dir_path);
                    for entry in &list {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let file_type = match entry.file_type() {
                            Ok(ft) if ft.is_dir() => "d",
                            Ok(ft) if ft.is_symlink() => "l",
                            Ok(_) => "-",
                            Err(_) => "?",
                        };
                        let size = entry.metadata()
                            .map(|m| m.len())
                            .unwrap_or(0);
                        println!("  {} {:>8} {}", file_type, size, name);
                    }
                    println!("  ({} entries)", list.len());
                    EXIT_ALLOWED
                }
                Err(e) => {
                    eprintln!("list-dir: {}", e);
                    EXIT_ERROR
                }
            }
        }
        "__builtin_stat" => {
            let path = args.trim();
            if path.is_empty() {
                eprintln!("Usage: stat <path>");
                return EXIT_ERROR;
            }
            match std::fs::metadata(path) {
                Ok(m) => {
                    let ftype = if m.is_dir() { "directory" }
                        else if m.is_symlink() { "symlink" }
                        else if m.is_file() { "file" }
                        else { "other" };
                    println!("File: {}", path);
                    println!("  Type: {}", ftype);
                    println!("  Size: {} bytes", m.len());
                    use std::os::unix::fs::PermissionsExt;
                    println!("  Permissions: {:o}", m.permissions().mode() & 0o777);
                    if let Ok(mtime) = m.modified() {
                        if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                            println!("  Modified: {} (epoch)", dur.as_secs());
                        }
                    }
                    EXIT_ALLOWED
                }
                Err(e) => {
                    eprintln!("stat: {}", e);
                    EXIT_ERROR
                }
            }
        }
        "__builtin_audit" => audit_cmd(args),
        "__builtin_reset" => reset_cmd(),
        // ── Agent memory builtins ─────────────────────────────────────────
        "__builtin_session_start" => crate::agent::cmd_session_start(args),
        "__builtin_session_status" => crate::agent::cmd_session_status(),
        "__builtin_session_end" => crate::agent::cmd_session_end(),
        "__builtin_session_goal" => crate::agent::cmd_session_goal(args),
        "__builtin_remember" => crate::agent::cmd_remember(args),
        "__builtin_recall" => crate::agent::cmd_recall(args),
        "__builtin_observe" => crate::agent::cmd_observe(args),
        "__builtin_forget" => crate::agent::cmd_forget(args),
        "__builtin_context_set" => crate::agent::cmd_context_set(args),
        "__builtin_context_get" => crate::agent::cmd_context_get(args),
        "__builtin_auto_attack" => auto_attack_cmd(),
        "__builtin_self_state" => { self_state_cmd(); config::EXIT_ALLOWED },
        "__builtin_health_check" => health_check_cmd(),
        "__builtin_proc_list" => proc_list_cmd(),
        _ => {
            eprintln!("Unknown builtin: {}", exec_target);
            EXIT_ERROR
        }
    }
}

// ── Layer 2: Auto-Attack ───────────────────────────────────────────────────

fn auto_attack_cmd() -> i32 {
    use std::process::Command;
    println!("Running auto-attack...");
    let candidates = ["../tests/auto-attack.sh", "../../tests/auto-attack.sh"];
    let mut found: Option<String> = None;
    for c in &candidates {
        if std::path::Path::new(c).exists() { found = Some(c.to_string()); break; }
    }
    let script = match found {
        Some(s) => s,
        None => {
            eprintln!("auto-attack: script not found");
            return config::EXIT_ERROR;
        }
    };
    match Command::new("sh").arg(&script).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", stdout.trim());
            if stdout.contains("0 failed") { config::EXIT_ALLOWED }
            else { config::EXIT_ERROR }
        }
        Err(e) => { eprintln!("auto-attack: {}", e); config::EXIT_ERROR }
    }
}


// ── Process management ─────────────────────────────────────────────────────

fn health_check_cmd() -> i32 {
    use crate::memory;
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut warnings: Vec<&str> = Vec::new();
    // Check recent memory saturation
    let recent_count = memory::recent_entries().len();
    if recent_count >= config::HEALTH_MEMORY_RECENT_MAX {
        warnings.push("recent memory near capacity");
    }
    // Check uptime
    if let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) {
        if dur.as_secs() > config::HEALTH_UPTIME_WARN {
            warnings.push("long session: consider reflection phase");
        }
    }
    if warnings.is_empty() {
        println!("HEALTH: PASS");
        config::EXIT_ALLOWED
    } else if warnings.len() <= 2 {
        println!("HEALTH: WARN");
        for w in &warnings { println!("  - {}", w); }
        config::EXIT_ALLOWED
    } else {
        println!("HEALTH: CRITICAL");
        for w in &warnings { println!("  - {}", w); }
        println!("  → agent should pause and recover before continuing");
        config::EXIT_ERROR
    }
}
fn self_state_cmd() {
    use std::time::{SystemTime, UNIX_EPOCH};
    println!("=== BoOS Self-State ===");
    // Session duration — whatever we can measure
    if let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) {
        println!("uptime: {}s", dur.as_secs());
    }
    // Memory stats
    if let Ok(wm) = crate::memory::WorkingMemory::load() {
        println!("goals: {}", wm.goals.len());
        println!("facts: {}", wm.active_facts.len());
        println!("context_keys: {}", wm.context.len());
    }
    // Recent memory count
    let recent = crate::memory::recent_entries();
    println!("recent_entries: {}", recent.len());
    // Context usage (approximate)
    let total_chars: usize = recent.iter().map(|e| e.content.len()).sum();
    println!("context_chars: {}", total_chars);
    // Attack status
    if std::path::Path::new("../tests/auto-attack.sh").exists() {
        println!("attack_verified: yes");
    }
    println!("=== End Self-State ===");
}
fn proc_list_cmd() -> i32 {
    use std::process::Command;
    println!("Running processes:");
    // Try /proc first (Linux), fall back to ps
    if let Ok(entries) = std::fs::read_dir("/proc") {
        let mut pids: Vec<u32> = Vec::new();
        for e in entries.filter_map(|e| e.ok()) {
            if let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() {
                pids.push(pid);
            }
        }
        pids.sort();
        for pid in pids {
            let cmdline = std::fs::read_to_string(format!("/proc/{}/comm", pid)).unwrap_or_default();
            println!("  {} {}", pid, cmdline.trim());
        }
    } else {
        let output = Command::new("ps").args(["-eo", "pid,comm"]).output();
        if let Ok(o) = output {
            println!("{}", String::from_utf8_lossy(&o.stdout).trim());
        }
    }
    config::EXIT_ALLOWED
}
// ── Audit functions ────────────────────────────────────────────────────────

fn audit_cmd(args: &str) -> i32 {
    let args = args.trim();
    if args.is_empty() {
        eprintln!("Usage: audit <recent|failures|session|summary> [args...]");
        return EXIT_ERROR;
    }
    let (subcmd, rest) = match args.find(|c: char| c.is_whitespace()) {
        Some(pos) => (&args[..pos], args[pos..].trim()),
        None => (args, ""),
    };
    match subcmd {
        "recent" => audit_recent(rest),
        "failures" => audit_failures(),
        "session" => audit_session(rest),
        "summary" => audit_summary(),
        _ => {
            eprintln!("Unknown audit subcommand: {}", subcmd);
            eprintln!("Usage: audit <recent|failures|session|summary>");
            EXIT_ERROR
        }
    }
}

fn audit_recent(args: &str) -> i32 {
    let n: usize = args.split_whitespace().next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let mut entries: Vec<(String, std::fs::Metadata)> = Vec::new();
    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                if let Ok(meta) = path.metadata() {
                    entries.push((path.to_string_lossy().to_string(), meta));
                }
            }
        }
    }
    entries.sort_by(|a, b| {
        let ta = a.1.modified().ok();
        let tb = b.1.modified().ok();
        tb.cmp(&ta)
    });

    println!("Recent {} actions:", n.min(entries.len()));
    for (path_str, _) in entries.iter().take(n) {
        let kv = registry::parse_kv_file(Path::new(path_str));
        let id = kv.get("id").map(|s| s.as_str()).unwrap_or("?");
        let cmd = kv.get("command").map(|s| s.as_str()).unwrap_or("?");
        let args = kv.get("args").map(|s| s.as_str()).unwrap_or("");
        let verdict = kv.get("verdict").map(|s| s.as_str()).unwrap_or("?");
        let session = kv.get("session_id").map(|s| s.as_str()).unwrap_or("");
        if args.is_empty() {
            println!("  {} {} -> {} (session: {})", id, cmd, verdict, session);
        } else {
            println!("  {} {} {} -> {} (session: {})", id, cmd, args, verdict, session);
        }
    }
    EXIT_ALLOWED
}

fn audit_failures() -> i32 {
    println!("Denied and errored actions:");
    let mut found = false;
    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                let kv = registry::parse_kv_file(&path);
                let verdict = kv.get("verdict").map(|s| s.as_str()).unwrap_or("");
                if verdict == "denied" || verdict == "error" || verdict == "unknown" {
                    let id = kv.get("id").map(|s| s.as_str()).unwrap_or("?");
                    let cmd = kv.get("command").map(|s| s.as_str()).unwrap_or("?");
                    let exit_code = kv.get("exit_code").map(|s| s.as_str()).unwrap_or("?");
                    println!("  {} {} -> {} (exit={})", id, cmd, verdict, exit_code);
                    found = true;
                }
            }
        }
    }
    if !found {
        println!("  (no failures)");
    }
    EXIT_ALLOWED
}

fn audit_session(session_id: &str) -> i32 {
    if session_id.is_empty() {
        eprintln!("Usage: audit session <session-id>");
        return EXIT_ERROR;
    }
    println!("Session: {}", session_id);
    let mut found = false;
    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                let kv = registry::parse_kv_file(&path);
                let sid = kv.get("session_id").map(|s| s.as_str()).unwrap_or("");
                if sid == session_id {
                    let id = kv.get("id").map(|s| s.as_str()).unwrap_or("?");
                    let cmd = kv.get("command").map(|s| s.as_str()).unwrap_or("?");
                    let verdict = kv.get("verdict").map(|s| s.as_str()).unwrap_or("?");
                    let exit_code = kv.get("exit_code").map(|s| s.as_str()).unwrap_or("?");
                    println!("  {} {} -> {} (exit={})", id, cmd, verdict, exit_code);
                    found = true;
                }
            }
        }
    }
    if !found {
        println!("  (no actions in this session)");
    }
    EXIT_ALLOWED
}

fn audit_summary() -> i32 {
    let mut total = 0u32;
    let mut allowed = 0u32;
    let mut denied = 0u32;
    let mut error = 0u32;
    let mut unknown = 0u32;

    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                let kv = registry::parse_kv_file(&path);
                total += 1;
                match kv.get("verdict").map(|s| s.as_str()).unwrap_or("") {
                    "allowed" => allowed += 1,
                    "denied" => denied += 1,
                    "error" => error += 1,
                    "unknown" => unknown += 1,
                    _ => {}
                }
            }
        }
    }

    println!("Audit Summary:");
    println!("  Total actions: {}", total);
    println!("  Allowed:       {}", allowed);
    println!("  Denied:        {}", denied);
    println!("  Errors:        {}", error);
    println!("  Unknown:       {}", unknown);

    if total > 0 {
        let pct = (allowed as f64 / total as f64) * 100.0;
        println!("  Success rate:  {:.1}%", pct);
    }

    EXIT_ALLOWED
}

/// Clear all persistent state: results, requests, logs, memory.
/// This is a human-only operation — allow_reset is 0 by default.
fn reset_cmd() -> i32 {
    println!("Resetting BoOS persistent state...");

    // 1. Clear results
    let mut cleared_results = 0u32;
    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            if e.path().extension().map_or(false, |ext| ext == "out") {
                if std::fs::remove_file(e.path()).is_ok() {
                    cleared_results += 1;
                }
            }
        }
    }
    println!("  Results cleared: {}", cleared_results);

    // 2. Clear requests
    let mut cleared_requests = 0u32;
    if let Ok(dir) = std::fs::read_dir(config::REQ_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            if std::fs::remove_file(e.path()).is_ok() {
                cleared_requests += 1;
            }
        }
    }
    println!("  Requests cleared: {}", cleared_requests);

    // 3. Rotate and truncate log
    rotate_logs_cmd();
    if let Ok(f) = std::fs::File::create(config::LOG_FILE) {
        drop(f);
    }
    println!("  Log reset");

    // 4. Clear memory
    let memory_dir = "/var/boos/memory";
    let mut cleared_memory = 0u32;
    if let Ok(entries) = std::fs::read_dir(memory_dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.is_dir() {
                if let Ok(sub) = std::fs::read_dir(&path) {
                    for se in sub.filter_map(|s| s.ok()) {
                        if std::fs::remove_file(se.path()).is_ok() {
                            cleared_memory += 1;
                        }
                    }
                }
            } else if std::fs::remove_file(&path).is_ok() {
                cleared_memory += 1;
            }
        }
    }
    println!("  Memory files cleared: {}", cleared_memory);

    // 5. Clear last-cmd
    let _ = std::fs::remove_file(config::LAST_CMD_FILE);

    log::log("boos-exec", "reset", &[
        ("results", &cleared_results.to_string()),
        ("requests", &cleared_requests.to_string()),
        ("memory", &cleared_memory.to_string()),
    ]);
    println!("Reset complete.");
    EXIT_ALLOWED
}

pub fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: boos-exec <command> [args...]");
        process::exit(EXIT_ERROR);
    }

    let cmd_name = &args[1];
    let cmd_args: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();
    let cmd_args_str = cmd_args.join(" ");

    let cmd = match registry::find_command(cmd_name) {
        Some(c) => c,
        None => {
            eprintln!("Unknown command: {}", cmd_name);
            log::log_unknown(cmd_name);
            process::exit(EXIT_UNKNOWN);
        }
    };

    if !check_enabled(&cmd.enable_flag, &cmd.name) {
        process::exit(EXIT_DENIED);
    }

    log::log_allowed(&cmd.name, &cmd.description);

    let _ = std::fs::create_dir_all(Path::new(config::LAST_CMD_FILE).parent().unwrap());
    let last_cmd = format!("{} {}", cmd_name, cmd_args_str);
    let _ = fs::write(config::LAST_CMD_FILE, last_cmd.trim());

    let exit_code = if cmd.exec.starts_with("__builtin_") {
        run_builtin(&cmd.exec, &cmd_args_str)
    } else {
        // External binary registered via `exec=/path/...`. Its exit code is
        // passed through verbatim; process.rs maps non-{0,1,3} → "error".
        match process::Command::new(&cmd.exec)
            .args(&args[2..])
            .status()
        {
            Ok(s) => s.code().unwrap_or(EXIT_ERROR),
            Err(e) => {
                eprintln!("Failed to execute {}: {}", cmd.exec, e);
                EXIT_ERROR
            }
        }
    };

    process::exit(exit_code);
}
