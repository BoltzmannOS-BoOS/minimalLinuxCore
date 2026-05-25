use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process;

use crate::config;
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
    println!("  shell               enter raw BusyBox shell");
    println!("  poweroff            power off system");
}

fn list_commands() {
    println!("Available registered commands:");
    for cmd in registry::load_commands() {
        println!("  {} — {}", cmd.name, cmd.description);
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
            0
        }
        _ => {
            eprintln!("Invalid level: {}. Use quiet, normal, or verbose.", level);
            1
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
            0
        }
        Err(_) => {
            eprintln!("No result found for: {}", id);
            1
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

/// Run a builtin command. Returns exit code.
fn run_builtin(exec_target: &str, args: &str) -> i32 {
    match exec_target {
        "__builtin_help" => { show_help(); 0 }
        "__builtin_commands" => { list_commands(); 0 }
        "__builtin_status" => { show_status(); 0 }
        "__builtin_log" => { show_log(); 0 }
        "__builtin_caps" => { show_caps(); 0 }
        "__builtin_debug" => {
            if args.is_empty() {
                show_debug();
                0
            } else {
                // Take first word as level
                let level = args.split_whitespace().next().unwrap_or("");
                set_debug(level)
            }
        }
        "__builtin_submit" => {
            if args.is_empty() {
                eprintln!("Usage: submit <command> [args...]");
                1
            } else {
                // Call boos-submit binary with the args
                let mut cmd = process::Command::new("/bin/boos-submit");
                for arg in args.split_whitespace() {
                    cmd.arg(arg);
                }
                match cmd.status() {
                    Ok(s) => s.code().unwrap_or(1),
                    Err(e) => { eprintln!("submit error: {}", e); 1 }
                }
            }
        }
        "__builtin_process" => {
            match process::Command::new("/bin/boos-process").status() {
                Ok(s) => s.code().unwrap_or(1),
                Err(_) => 1,
            }
        }
        "__builtin_results" => { show_results(); 0 }
        "__builtin_result" => {
            if args.is_empty() {
                eprintln!("Usage: result <id>");
                1
            } else {
                let id = args.split_whitespace().next().unwrap_or("");
                show_result_by_id(id)
            }
        }
        "__builtin_shell" => {
            println!("Entering raw shell (type 'exit' to return)...");
            let child = process::Command::new("/bin/sh").spawn();
            match child {
                Ok(mut c) => { let _ = c.wait(); 0 }
                Err(e) => { eprintln!("shell error: {}", e); 1 }
            }
        }
        "__builtin_daemons" => {
            match process::Command::new("/bin/boos-supervisor")
                .arg("status")
                .status()
            {
                Ok(s) => s.code().unwrap_or(1),
                Err(_) => 1,
            }
        }
        "__builtin_poweroff" => {
            println!("Powering off...");
            let _ = process::Command::new("/bin/poweroff").arg("-f").status();
            0
        }
        _ => {
            eprintln!("Unknown builtin: {}", exec_target);
            1
        }
    }
}

pub fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: boos-exec <command> [args...]");
        process::exit(1);
    }

    let cmd_name = &args[1];
    let cmd_args: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();
    let cmd_args_str = cmd_args.join(" ");

    // Look up command in registry
    let cmd = match registry::find_command(cmd_name) {
        Some(c) => c,
        None => {
            eprintln!("Unknown command: {}", cmd_name);
            log::log_unknown(cmd_name);
            process::exit(1);
        }
    };

    // Capability check (using enable flag)
    if !check_enabled(&cmd.enable_flag, &cmd.name) {
        process::exit(1);
    }

    // Log allowed execution
    log::log_allowed(&cmd.name, &cmd.description);

    // Update last-cmd for command chaining
    let _ = std::fs::create_dir_all(Path::new(config::LAST_CMD_FILE).parent().unwrap());
    let last_cmd = format!("{} {}", cmd_name, cmd_args_str);
    let _ = fs::write(config::LAST_CMD_FILE, last_cmd.trim());

    // Dispatch
    let exit_code = if cmd.exec.starts_with("__builtin_") {
        run_builtin(&cmd.exec, &cmd_args_str)
    } else {
        // External executable (only if explicitly registered with exec=)
        match process::Command::new(&cmd.exec)
            .args(&args[2..])
            .status()
        {
            Ok(s) => s.code().unwrap_or(1),
            Err(e) => {
                eprintln!("Failed to execute {}: {}", cmd.exec, e);
                1
            }
        }
    };

    process::exit(exit_code);
}
