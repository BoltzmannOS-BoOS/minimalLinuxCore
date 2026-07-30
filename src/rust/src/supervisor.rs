use std::collections::HashMap;
use std::fs;
use std::io;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use crate::config;
use crate::log;
use crate::registry;

const DAEMON_DIR: &str = "/etc/boos/daemons";
const DAEMON_RUN_DIR: &str = "/var/boos/daemons";
const DAEMON_CONF: &str = "/etc/boos/daemon.conf";
const CAP_CONF: &str = "/etc/boos/capabilities.conf";
const MAX_RESTARTS: u32 = 5;
const DEFAULT_POLL_INTERVAL: u64 = 1;
const HEALTH_CHECK_INTERVAL: u64 = 2;

/// Get file modification time as seconds since epoch, or None.
fn mtime_secs(path: &str) -> Option<u64> {
    fs::metadata(path).ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

struct DaemonConfig {
    name: String,
    exec: String,
    user: String,   // optional — if set, run daemon as this user
    principal: String,
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
        match daemon_config_from_fields(&kv) {
            Ok(config) => configs.push(config),
            Err(error) => {
                log::log("boos-supervisor", "error", &[
                    ("msg", "invalid daemon config"),
                    ("file", &fname),
                    ("error", &log::json_escape(&error.to_string())),
                ]);
            }
        }
    }

    configs
}

fn daemon_config_from_fields(fields: &HashMap<String, String>) -> io::Result<DaemonConfig> {
    let name = fields.get("name").cloned().unwrap_or_default();
    let exec = fields.get("exec").cloned().unwrap_or_default();
    if name.is_empty() || exec.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "daemon name and exec are required",
        ));
    }

    let principal = fields.get("principal").cloned().unwrap_or_default();
    if !principal.is_empty() && !config::is_valid_runtime_id(&principal) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "daemon principal is invalid",
        ));
    }

    Ok(DaemonConfig {
        name,
        exec,
        user: fields.get("user").cloned().unwrap_or_default(),
        principal,
        restart: fields
            .get("restart")
            .cloned()
            .unwrap_or_else(|| "always".into()),
        enabled: fields.get("enabled").map(|value| value == "1").unwrap_or(false),
    })
}

fn privilege_dropped_command(daemon: &DaemonConfig) -> String {
    if daemon.principal.is_empty() {
        daemon.exec.clone()
    } else {
        format!(
            "BOOS_PRINCIPAL_ID={} {}",
            daemon.principal, daemon.exec
        )
    }
}

fn spawn_daemon_process(daemon: &DaemonConfig) -> io::Result<Child> {
    let parts: Vec<&str> = daemon.exec.split_whitespace().collect();
    if parts.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "daemon command is empty",
        ));
    }

    if !daemon.user.is_empty() {
        return Command::new("su")
            .args(["-c", &privilege_dropped_command(daemon), &daemon.user])
            .spawn();
    }

    let mut command = Command::new(parts[0]);
    command.args(&parts[1..]);
    if !daemon.principal.is_empty() {
        command.env("BOOS_PRINCIPAL_ID", &daemon.principal);
    }
    command.spawn()
}

fn spawn_daemon(d: &DaemonConfig, children: &mut HashMap<String, ChildInfo>) {
    log::log("boos-supervisor", "starting", &[
        ("daemon", &d.name),
        ("cmd", &d.exec),
        ("principal", &d.principal),
    ]);

    match spawn_daemon_process(d) {
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

    match spawn_daemon_process(d) {
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

    // Spawn enabled daemons
    for d in &daemons {
        if d.enabled {
            if !children.contains_key(&d.name) {
                spawn_daemon(d, &mut children);
            }
        }
    }

    let mut last_health_check = Instant::now();
    let mut last_daemon_conf_mtime = mtime_secs(DAEMON_CONF);
    let mut last_cap_conf_mtime = mtime_secs(CAP_CONF);
    let mut poll_interval = load_poll_interval();
    let processing = Arc::new(AtomicBool::new(false));

    // Main supervision + polling loop
    loop {
        // Hot reload: check if daemon.conf changed
        let current_mtime = mtime_secs(DAEMON_CONF);
        if current_mtime != last_daemon_conf_mtime {
            last_daemon_conf_mtime = current_mtime;
            let new_interval = load_poll_interval();
            if new_interval != poll_interval {
                log::log("boos-supervisor", "config_reload", &[
                    ("file", DAEMON_CONF),
                    ("old_poll", &poll_interval.to_string()),
                    ("new_poll", &new_interval.to_string()),
                ]);
                poll_interval = new_interval;
            }
        }

        // Hot reload: capabilities.conf
        let cap_mtime = mtime_secs(CAP_CONF);
        if cap_mtime != last_cap_conf_mtime {
            last_cap_conf_mtime = cap_mtime;
            log::log("boos-supervisor", "config_reload", &[
                ("file", CAP_CONF),
            ]);
        }

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

        // Process request queue in background; skip if previous run still in flight
        if processing.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            let flag = Arc::clone(&processing);
            std::thread::spawn(move || {
                crate::process::main();
                flag.store(false, Ordering::SeqCst);
            });
        }

        std::thread::sleep(Duration::from_secs(poll_interval));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_is_exported_to_a_privilege_dropped_daemon() {
        let daemon = DaemonConfig {
            name: "agent".to_string(),
            exec: "/bin/boos-agent resident".to_string(),
            user: "boos-agent".to_string(),
            principal: "resident".to_string(),
            restart: "always".to_string(),
            enabled: true,
        };

        assert_eq!(
            privilege_dropped_command(&daemon),
            "BOOS_PRINCIPAL_ID=resident /bin/boos-agent resident"
        );
    }

    #[test]
    fn invalid_principal_cannot_enter_a_daemon_command() {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "agent".to_string());
        fields.insert("exec".to_string(), "/bin/boos-agent resident".to_string());
        fields.insert("user".to_string(), "boos-agent".to_string());
        fields.insert("principal".to_string(), "resident;reboot".to_string());
        fields.insert("restart".to_string(), "always".to_string());
        fields.insert("enabled".to_string(), "1".to_string());

        assert!(daemon_config_from_fields(&fields).is_err());
    }
}
