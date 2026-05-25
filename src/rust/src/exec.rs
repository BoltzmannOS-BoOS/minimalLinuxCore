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
fn check_enabled(flag: &str, name: &str) -> bool {
    if registry::is_enabled(flag) {
        return true;
    }
    println!("Permission denied: missing capability '{}'", name);
    // Use the enable_flag key for the log as-is (allow_*)
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
                Err(_) => EXIT_ERROR,
            }
        }
        "__builtin_poweroff" => {
            println!("Powering off...");
            let _ = process::Command::new("/bin/poweroff").arg("-f").status();
            EXIT_ALLOWED
        }
        "__builtin_prune" => prune_results(args),
        "__builtin_rotate_logs" => rotate_logs_cmd(),
        _ => {
            eprintln!("Unknown builtin: {}", exec_target);
            EXIT_ERROR
        }
    }
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
