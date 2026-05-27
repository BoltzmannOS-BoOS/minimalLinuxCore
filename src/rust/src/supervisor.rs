use std::collections::HashMap;
use std::fs;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::log;
use crate::registry;

const DAEMON_DIR: &str = "/etc/boos/daemons";
const DAEMON_RUN_DIR: &str = "/var/boos/daemons";
const MAX_RESTARTS: u32 = 5;
const DEFAULT_POLL_INTERVAL: u64 = 1; // seconds
const HEALTH_CHECK_INTERVAL: u64 = 2; // seconds

struct DaemonConfig {
    name: String,
    exec: String,
    restart: String, // "always" or "never"
    enabled: bool,
}

struct ChildInfo {
    child: Child,
    restarts: u32,
}

fn load_poll_interval() -> u64 {
    let conf_path = "/etc/boos/daemon.conf";
    if let Ok(content) = fs::read_to_string(conf_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("POLL_INTERVAL=") {
                if let Ok(val) = line["POLL_INTERVAL=".len()..].parse::<u64>() {
                    if val > 0 {
                        return val;
                    }
                }
            }
        }
    }
    DEFAULT_POLL_INTERVAL
}

fn load_daemon_configs() -> Vec<DaemonConfig> {
    let mut configs = Vec::new();
    let dir = match fs::read_dir(DAEMON_DIR) {
        Ok(d) => d,
        Err(e) => {
            log::log("boos-supervisor", "error", &[
                ("msg", "cannot read daemon config dir"),
                ("error", &e.to_string()),
            ]);
            return configs;
        }
    };

    for entry in dir.flatten() {
        let path = entry.path();
        let fname = path.to_string_lossy();
        if !fname.ends_with(".daemon") {
            continue;
        }

        let kv = registry::parse_kv_file(&path);
        let name = kv.get("name").cloned().unwrap_or_default();
        let exec = kv.get("exec").cloned().unwrap_or_default();
        let restart = kv.get("restart").cloned().unwrap_or_else(|| "always".into());
        let enabled = kv.get("enabled").map(|v| v == "1").unwrap_or(false);

        if name.is_empty() || exec.is_empty() {
            log::log("boos-supervisor", "error", &[
                ("msg", "invalid daemon config"),
                ("file", &fname),
            ]);
            continue;
        }

        configs.push(DaemonConfig { name, exec, restart, enabled });
    }

    configs
}

fn spawn_daemon(d: &DaemonConfig, children: &mut HashMap<String, ChildInfo>) {
    let parts: Vec<&str> = d.exec.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    let cmd = parts[0];
    let args = &parts[1..];

    log::log("boos-supervisor", "starting", &[
        ("daemon", &d.name),
        ("cmd", &d.exec),
    ]);

    match Command::new(cmd).args(args).spawn() {
        Ok(child) => {
            let pid = child.id();
            children.insert(d.name.clone(), ChildInfo {
                child,
                restarts: 0,
            });
            log::log("boos-supervisor", "started", &[
                ("daemon", &d.name),
                ("pid", &pid.to_string()),
            ]);
        }
        Err(e) => {
            log::log("boos-supervisor", "error", &[
                ("daemon", &d.name),
                ("msg", "failed to spawn"),
                ("error", &e.to_string()),
            ]);
        }
    }
}

fn check_and_restart(d: &DaemonConfig, children: &mut HashMap<String, ChildInfo>) {
    let needs_restart = match children.get_mut(&d.name) {
        Some(info) => {
            match info.child.try_wait() {
                Ok(Some(status)) => {
                    // Child exited
                    log::log("boos-supervisor", "exited", &[
                        ("daemon", &d.name),
                        ("status", &status.to_string()),
                    ]);
                    true
                }
                Ok(None) => {
                    // Still running
                    false
                }
                Err(e) => {
                    // Error checking — treat as dead
                    log::log("boos-supervisor", "error", &[
                        ("daemon", &d.name),
                        ("msg", "try_wait failed"),
                        ("error", &e.to_string()),
                    ]);
                    true
                }
            }
        }
        None => {
            // Not in our map — was never spawned or was removed
            true
        }
    };

    if !needs_restart {
        return;
    }

    if d.restart == "never" {
        log::log("boos-supervisor", "stopped", &[
            ("daemon", &d.name),
            ("restart_policy", "never"),
        ]);
        children.remove(&d.name);
        return;
    }

    // Check restart limit
    let restarts = children.get(&d.name).map(|i| i.restarts).unwrap_or(0);
    if restarts >= MAX_RESTARTS {
        log::log("boos-supervisor", "failed", &[
            ("daemon", &d.name),
            ("reason", "max_restarts"),
            ("max", &MAX_RESTARTS.to_string()),
        ]);
        children.remove(&d.name);
        return;
    }

    // Restart
    let count = restarts + 1;
    log::log("boos-supervisor", "restarting", &[
        ("daemon", &d.name),
        ("attempt", &count.to_string()),
        ("reason", "process_died"),
    ]);

    // Remove old entry
    children.remove(&d.name);

    // Spawn new process
    let parts: Vec<&str> = d.exec.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }
    let cmd = parts[0];
    let args = &parts[1..];

    match Command::new(cmd).args(args).spawn() {
        Ok(child) => {
            children.insert(d.name.clone(), ChildInfo {
                child,
                restarts: count,
            });
            log::log("boos-supervisor", "started", &[
                ("daemon", &d.name),
                ("attempt", &count.to_string()),
            ]);
        }
        Err(e) => {
            log::log("boos-supervisor", "error", &[
                ("daemon", &d.name),
                ("msg", "restart spawn failed"),
                ("error", &e.to_string()),
            ]);
        }
    }
}

fn show_status() {
    let configs = load_daemon_configs();
    let mut found = false;

    println!("Daemon status:");
    for d in &configs {
        found = true;
        if !d.enabled {
            println!("  {}: disabled", d.name);
            continue;
        }

        // Check /proc for running instances matching daemon name
        // (no PID file needed — we just check if any process cmdline matches)
        let running = is_daemon_running(&d.name);
        if running {
            println!("  {}: running", d.name);
        } else {
            println!("  {}: stopped", d.name);
        }
    }

    if !found {
        println!("  (no daemon configs found)");
    }
}

fn is_daemon_running(name: &str) -> bool {
    if let Ok(procs) = fs::read_dir("/proc") {
        for entry in procs.flatten() {
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy();
            // Only process directories (numeric names)
            if !fname_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let cmdline_path = entry.path().join("cmdline");
            if let Ok(data) = fs::read(&cmdline_path) {
                // cmdline is null-separated; split and check each arg
                // argv[0] is the binary path — check if name appears in it
                for part in data.split(|&b| b == 0) {
                    let s = String::from_utf8_lossy(part);
                    if s.is_empty() {
                        continue;
                    }
                    if s.contains(name) {
                        return true;
                    }
                    break; // Only check argv[0] (first non-empty segment)
                }
            }
        }
    }
    false
}

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "status" {
        show_status();
        return;
    }

    // Ensure run dir exists
    let _ = fs::create_dir_all(DAEMON_RUN_DIR);

    log::log("boos-supervisor", "started", &[("mode", "supervise")]);

    let daemons = load_daemon_configs();
    let mut children: HashMap<String, ChildInfo> = HashMap::new();
    let poll_interval = load_poll_interval();

    // Spawn enabled daemons
    for d in &daemons {
        if d.enabled {
            if !children.contains_key(&d.name) {
                spawn_daemon(d, &mut children);
            }
        }
    }

    let mut last_health_check = Instant::now();

    // Main supervision + polling loop
    loop {
        // Health check every HEALTH_CHECK_INTERVAL seconds
        if last_health_check.elapsed() >= Duration::from_secs(HEALTH_CHECK_INTERVAL) {
            for d in &daemons {
                if !d.enabled {
                    continue;
                }
                check_and_restart(d, &mut children);
            }
            last_health_check = Instant::now();
        }

        // Process request queue (absorb boos-daemon role)
        // Call boos-process directly since we're in the same multi-call binary
        crate::process::main();

        std::thread::sleep(Duration::from_secs(poll_interval));
    }
}
