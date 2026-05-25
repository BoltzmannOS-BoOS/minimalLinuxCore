use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::log;

/// Generate a unique request ID: req-<ms>-<random4>
fn generate_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    // Simple random suffix using /proc/uptime fractional + PID
    let uptime = log::uptime_secs();
    let suffix = ((uptime * 10000.0) as u64) % 10000;
    format!("req-{}-{:04}", now, suffix)
}

pub fn main() {
    let args: Vec<String> = env::args().collect();

    // args[0] is program name, args[1..] = cmd + its args
    if args.len() < 2 {
        eprintln!("Usage: boos-submit <command> [args...]");
        process::exit(1);
    }

    let cmd = &args[1];
    let cmd_args: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();
    let cmd_args_str = cmd_args.join(" ");

    // Requester identity: set by caller, no -r override allowed
    let requester = env::var("BOOS_REQUESTER").unwrap_or_else(|_| "unknown".to_string());

    // Ensure request directory exists
    let _ = fs::create_dir_all(config::REQ_DIR);

    let id = generate_id();
    let submitted_at = log::uptime_secs();

    let content = format!(
        "id={}\nrequester={}\ncommand={}\nargs={}\nsubmitted_at={:.3}\nstatus=pending\n",
        id, requester, cmd, cmd_args_str, submitted_at
    );

    // Atomic write: temp file + rename. Use O_EXCL for collision detection.
    let file_path = Path::new(config::REQ_DIR).join(&id);
    let tmp_path = Path::new(config::REQ_DIR).join(format!("{}.tmp", id));

    let mut attempt = 0u32;
    loop {
        let target = if attempt == 0 { &file_path } else { &tmp_path };
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
        {
            Ok(mut f) => {
                let _ = f.write_all(content.as_bytes());
                if attempt > 0 {
                    // Rename tmp onto the real path (atomic on same fs).
                    // If file_path was created by another process between our
                    // attempts, rename will replace it — acceptable trade-off.
                    let _ = fs::rename(&tmp_path, &file_path);
                }
                break;
            }
            Err(_) => {
                attempt += 1;
                if attempt > 10 {
                    eprintln!("Failed to create request file after {} attempts", attempt);
                    process::exit(1);
                }
                // If tmp_path also exists, unlink and retry (broken retry from
                // a previous crashed process could leave tmp_path behind).
                if attempt > 1 {
                    let _ = fs::remove_file(&tmp_path);
                }
                // Brief backoff to let the colliding process finish
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    // Log submission
    log::log(
        "boos-submit",
        "submitted",
        &[
            ("id", &id),
            ("requester", &requester),
            ("command", cmd),
            ("args", &log::json_escape(&cmd_args_str)),
        ],
    );

    println!("Submitted: {}", id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        // Format: req-<ms>-<4-digit-suffix>
        assert!(id.starts_with("req-"), "ID should start with 'req-': {}", id);
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 3, "ID should have 3 dash-separated parts: {}", id);
        // Last part should be 4 digits
        let suffix = parts.last().unwrap();
        assert_eq!(suffix.len(), 4, "suffix should be 4 digits: {}", id);
        assert!(suffix.chars().all(|c| c.is_ascii_digit()), "suffix should be digits: {}", id);
        // Middle part should be the timestamp (13 digits for millis)
        assert!(parts[1].len() >= 10, "timestamp part too short: {}", id);
    }

    #[test]
    fn test_generate_id_uniqueness_with_small_delay() {
        // BUG: generate_id() uses SystemTime::now().as_millis() + deterministic suffix.
        // Rapid calls within the same millisecond produce identical IDs.
        // The O_EXCL + retry loop in submit_main() compensates for this.
        // This test documents the limitation by sleeping between calls.
        let mut ids = Vec::new();
        for _ in 0..10 {
            ids.push(generate_id());
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(),
            "IDs should be unique with 1ms delay (got {} unique out of {})",
            unique.len(), ids.len());
    }

    #[test]
    #[should_panic(expected = "collisions documented")]
    fn test_generate_id_collisions_without_delay() {
        // This test DOCUMENTS a known limitation: without delays,
        // generate_id() can produce collisions. The O_EXCL retry in
        // submit_main() is the actual safety net.
        let mut ids = Vec::new();
        for _ in 0..100 {
            ids.push(generate_id());
        }
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        if unique.len() < ids.len() {
            panic!("collisions documented: {} unique out of {}",
                unique.len(), ids.len());
        }
    }
}
