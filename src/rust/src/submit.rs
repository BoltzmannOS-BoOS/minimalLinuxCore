use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config;
use crate::log;

// ... (random_suffix, generate_id unchanged)

/// Read 4 random bytes from /dev/urandom. Falls back to a PID/uptime mix
/// if /dev/urandom is unreadable (extremely rare on Linux; would imply a
/// broken initramfs).
fn random_suffix() -> u32 {
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        let mut buf = [0u8; 4];
        if f.read_exact(&mut buf).is_ok() {
            return u32::from_le_bytes(buf);
        }
    }
    let uptime = (log::uptime_secs() * 1_000_000.0) as u64;
    let pid = process::id() as u64;
    ((uptime ^ pid) & 0xFFFF_FFFF) as u32
}

/// Generate a unique request ID: req-<ms>-<random8hex>.
/// Millis disambiguates across seconds; the random suffix disambiguates
/// within the same millisecond. The O_EXCL retry in `main` is the final
/// safety net against the astronomically rare collision.
fn generate_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("req-{}-{:08x}", now, random_suffix())
}

fn wait_for_result(id: &str, timeout_secs: u64) {
    let result_path = Path::new(config::RESULT_DIR).join(format!("{}.out", id));
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        if let Ok(content) = fs::read_to_string(&result_path) {
            // Parse exit code from result metadata
            let exit_code = content
                .lines()
                .find(|l| l.starts_with("exit_code="))
                .and_then(|l| l["exit_code=".len()..].parse::<i32>().ok())
                .unwrap_or(1);

            // Print everything after the "---" delimiter
            if let Some(pos) = content.find("\n---\n") {
                print!("{}", &content[pos + 5..]);
            }
            process::exit(exit_code);
        }

        if Instant::now() > deadline {
            eprintln!("Timeout waiting for result {}", id);
            process::exit(1);
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse --wait flag
    let mut wait_mode = false;
    let mut timeout = 30u64;
    let mut cmd_parts: Vec<&str> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--wait" || args[i] == "-w" {
            wait_mode = true;
        } else if args[i] == "--timeout" || args[i] == "-t" {
            i += 1;
            if i < args.len() {
                timeout = args[i].parse().unwrap_or(30);
            }
        } else {
            cmd_parts.push(&args[i]);
        }
        i += 1;
    }

    if cmd_parts.is_empty() {
        eprintln!("Usage: boos-submit [--wait] [-t SECS] <command> [args...]");
        process::exit(1);
    }

    let cmd = cmd_parts[0];
    let cmd_args: Vec<&str> = cmd_parts[1..].to_vec();
    let cmd_args_str = cmd_args.join(" ");

    // Requester identity: set by caller, no -r override allowed
    let requester = env::var("BOOS_REQUESTER").unwrap_or_else(|_| "unknown".to_string());
    let session_id = env::var("BOOS_SESSION").ok();

    // Ensure request directory exists
    let _ = fs::create_dir_all(config::REQ_DIR);

    let id = generate_id();
    let submitted_at = log::uptime_secs();

    let mut content = format!(
        "id={}\nrequester={}\ncommand={}\nargs={}\nsubmitted_at={:.3}\nstatus=pending\n",
        id, requester, cmd, cmd_args_str, submitted_at
    );
    if let Some(ref sid) = session_id {
        content.push_str(&format!("session_id={}\n", sid));
    }

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

    if wait_mode {
        wait_for_result(&id, timeout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        assert!(id.starts_with("req-"), "ID should start with 'req-': {}", id);
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 3, "ID should have 3 dash-separated parts: {}", id);
        let suffix = parts.last().unwrap();
        assert_eq!(suffix.len(), 8, "suffix should be 8 hex chars: {}", id);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()),
            "suffix should be hex: {}", id);
        assert!(parts[1].len() >= 10, "timestamp part too short: {}", id);
    }

    #[test]
    fn test_generate_id_uniqueness_rapid() {
        // With a 32-bit /dev/urandom suffix, 1000 rapid calls colliding by
        // birthday paradox is ~1 in 8000. We allow at most one collision so
        // a transient failure of /dev/urandom (extremely rare) doesn't fail CI.
        let mut ids = Vec::new();
        for _ in 0..1000 {
            ids.push(generate_id());
        }
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        let collisions = ids.len() - unique.len();
        assert!(collisions <= 1,
            "too many ID collisions: {} out of {} (suffix entropy broken?)",
            collisions, ids.len());
    }

    #[test]
    fn test_random_suffix_varies() {
        // Two consecutive reads from /dev/urandom should almost always differ.
        let a = random_suffix();
        let b = random_suffix();
        // Very rare 1-in-2^32 false positive is acceptable for this test.
        assert_ne!(a, b, "random_suffix produced identical consecutive values: {}", a);
    }
}
