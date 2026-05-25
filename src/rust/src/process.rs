use std::fs;
use std::io::{self, Read};
use std::os::unix::process::ExitStatusExt as _;
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::log;
use crate::registry;

/// Execute a command and capture its output, enforcing MAX_OUTPUT_BYTES limit.
/// Returns (stdout, exit_code, was_truncated).
///
/// Reads stdout and stderr concurrently via threads to avoid pipe-buffer deadlock:
/// if the child fills stderr before stdout, sequential read would block forever.
fn capture_output(cmd: &str, args: &[&str]) -> (String, i32, bool) {
    let mut child = match process::Command::new(cmd)
        .args(args)
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (format!("Failed to spawn: {}", e), 1, false),
    };

    // Take ownership of pipes for thread-based concurrent reads
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let limit = config::MAX_OUTPUT_BYTES;

    // Read stdout in a thread
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::with_capacity(65536);
        let mut truncated = false;
        if let Some(pipe) = stdout_pipe {
            let _ = pipe.take(limit as u64 + 1).read_to_end(&mut buf);
            if buf.len() > limit {
                truncated = true;
                buf.truncate(limit);
            }
        }
        (buf, truncated)
    });

    // Read stderr in a thread
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut truncated = false;
        if let Some(pipe) = stderr_pipe {
            let _ = pipe.take(limit as u64 + 1).read_to_end(&mut buf);
            if buf.len() > limit {
                truncated = true;
                buf.truncate(limit);
            }
        }
        (buf, truncated)
    });

    let (stdout_buf, stdout_trunc) = stdout_handle.join().unwrap_or((Vec::new(), false));
    let (stderr_buf, stderr_trunc) = stderr_handle.join().unwrap_or((Vec::new(), false));
    let truncated = stdout_trunc || stderr_trunc;

    // Wait for child after pipes are drained
    let status = child.wait().unwrap_or_else(|_| {
        process::ExitStatus::from_raw(0x0100) // exit code 1 in raw wait status
    });
    let exit_code = status.code().unwrap_or(1);

    // Combine stdout and stderr
    let mut output = String::from_utf8_lossy(&stdout_buf).to_string();
    let err_out = String::from_utf8_lossy(&stderr_buf);
    if !err_out.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&err_out);
    }

    if truncated {
        output.push_str(&format!("\n[truncated {}+ bytes]", limit));
    }

    (output, exit_code, truncated)
}

/// Scan /var for files modified since the marker (verbose mode fs tracking).
fn files_changed_since(marker_ts: f64) -> String {
    let mut touched = Vec::new();
    let _ = walk_dir(Path::new("/var"), &mut touched, marker_ts, 0);
    touched.join(" ")
}

fn walk_dir(dir: &Path, result: &mut Vec<String>, since: f64, depth: u32) -> io::Result<()> {
    if depth > 10 {
        return Ok(());
    }
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.to_string_lossy().to_string();

            // Skip the daemon run-dir — supervisor writes PID files here on
            // every health check, which would otherwise show up as fs noise.
            if name.contains("/var/boos/daemons") {
                continue;
            }

            if path.is_file() {
                if let Ok(meta) = path.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        use std::time::UNIX_EPOCH;
                        if let Ok(t) = mtime.duration_since(UNIX_EPOCH) {
                            let ts = t.as_secs_f64();
                            if ts >= since {
                                result.push(name);
                            }
                        }
                    }
                }
            } else if path.is_dir() {
                walk_dir(&path, result, since, depth + 1)?;
            }
        }
    }
    Ok(())
}

pub fn main() {
    // Ensure directories exist
    let _ = fs::create_dir_all(config::REQ_DIR);
    let _ = fs::create_dir_all(config::RESULT_DIR);

    let trace = log::get_trace_level();
    let mut processed = 0u32;

    let dir = match fs::read_dir(config::REQ_DIR) {
        Ok(d) => d,
        Err(_) => {
            println!("No pending requests.");
            return;
        }
    };

    let mut entries: Vec<_> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("req-"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let _content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                let _ = fs::remove_file(&path);
                continue;
            }
        };

        let kv = registry::parse_kv_file(&path);
        let id = kv.get("id").cloned().unwrap_or_else(|| {
            entry.file_name().to_string_lossy().to_string()
        });
        let cmd = match kv.get("command").cloned() {
            Some(c) if !c.is_empty() => c,
            _ => {
                log::log("boos-process", "invalid_request", &[
                    ("file", &entry.file_name().to_string_lossy())
                ]);
                let _ = fs::remove_file(&path);
                continue;
            }
        };
        let args = kv.get("args").map(|s| s.as_str()).unwrap_or("");
        let requester = kv.get("requester").map(|s| s.as_str()).unwrap_or("unknown");

        let started_at = log::uptime_secs();
        let prev_cmd = fs::read_to_string(config::LAST_CMD_FILE).unwrap_or_default();
        let prev_cmd = prev_cmd.trim();

        // Verbose: record a baseline timestamp so we can scan /var for files
        // modified during execution. ext2 stores integer-second mtimes, so we
        // subtract 1s to avoid missing files whose mtime rounds down to the
        // same second as `now`. Trade-off: at most ~1s of pre-execution
        // changes may show up as false positives — acceptable for trace data.
        let marker_ts = if trace == log::TraceLevel::Verbose {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64() - 1.0)
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // Log execution start
        if trace == log::TraceLevel::Verbose {
            log::append_log_line(&format!(
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"executing\",\"id\":\"{}\",\"requester\":\"{}\",\"command\":\"{}\",\"args\":\"{}\",\"prev\":\"{}\"}}",
                started_at, log::json_escape(&id), log::json_escape(requester),
                log::json_escape(&cmd), log::json_escape(args), log::json_escape(prev_cmd)
            ));
        } else {
            log::append_log_line(&format!(
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"executing\",\"id\":\"{}\",\"requester\":\"{}\",\"command\":\"{}\",\"args\":\"{}\"}}",
                started_at, log::json_escape(&id), log::json_escape(requester),
                log::json_escape(&cmd), log::json_escape(args)
            ));
        }

        // Build full command: cmd + args (space-separated for boos-exec)
        let full_cmd = if args.is_empty() {
            cmd.clone()
        } else {
            format!("{} {}", cmd, args)
        };
        let cmd_parts: Vec<&str> = full_cmd.split_whitespace().collect();
        let exec_cmd = cmd_parts.first().map(|s| *s).unwrap_or("help");
        let exec_args = &cmd_parts[1..];

        let (output, exit_code, _truncated) = capture_output("/bin/boos-exec", &{
            let mut v = vec![exec_cmd];
            v.extend_from_slice(exec_args);
            v
        });

        let finished_at = log::uptime_secs();
        let duration = log::duration_ms(started_at, finished_at);

        // Map exit code → verdict using the contract from config.rs.
        // External programs invoked via `exec=` may return arbitrary codes;
        // anything outside {0,1,3} is recorded as "error".
        let verdict = match exit_code {
            config::EXIT_ALLOWED => "allowed",
            config::EXIT_DENIED => "denied",
            config::EXIT_UNKNOWN => "unknown",
            _ => "error",
        };

        let files_touched = if trace == log::TraceLevel::Verbose && marker_ts > 0.0 {
            let found = files_changed_since(marker_ts);
            if !found.is_empty() {
                log::append_log_line(&format!(
                    "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"fs_trace\",\"files\":\"{}\"}}",
                    log::uptime_secs(), log::json_escape(&found)
                ));
            }
            found
        } else {
            String::new()
        };

        // Update last-cmd
        let last = format!("{} {}", cmd, args);
        let _ = fs::write(config::LAST_CMD_FILE, last.trim());

        // Write result file (atomic)
        let result_path = Path::new(config::RESULT_DIR).join(format!("{}.out", id));
        let tmp_path = Path::new(config::RESULT_DIR).join(format!("{}.tmp", id));

        let mut result_content = format!(
            "id={}\nrequester={}\ncommand={}\nargs={}\nverdict={}\nexit_code={}\nstarted_at={:.3}\nfinished_at={:.3}\nduration_ms={}\n",
            id, requester, cmd, args, verdict, exit_code, started_at, finished_at, duration
        );

        if !prev_cmd.is_empty() {
            result_content.push_str(&format!("prev_command={}\n", prev_cmd));
        }
        if !files_touched.is_empty() {
            result_content.push_str(&format!("files_touched={}\n", files_touched));
        }

        result_content.push_str("---\n");
        result_content.push_str(&output);
        result_content.push('\n');

        // Atomic write
        let _ = fs::write(&tmp_path, &result_content);
        let _ = fs::rename(&tmp_path, &result_path);

        // Log completion
        if trace == log::TraceLevel::Verbose {
            log::append_log_line(&format!(
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"completed\",\"id\":\"{}\",\"verdict\":\"{}\",\"exit_code\":{},\"duration_ms\":{},\"files\":\"{}\"}}",
                log::uptime_secs(), log::json_escape(&id), verdict, exit_code, duration,
                log::json_escape(&files_touched)
            ));
        } else {
            log::append_log_line(&format!(
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"completed\",\"id\":\"{}\",\"verdict\":\"{}\",\"exit_code\":{},\"duration_ms\":{}}}",
                log::uptime_secs(), log::json_escape(&id), verdict, exit_code, duration
            ));
        }

        println!("[{}] {} ({}ms)", id, verdict, duration);
        println!("{}", output);

        // Remove request file
        let _ = fs::remove_file(&path);
        processed += 1;
    }

    if processed == 0 {
        println!("No pending requests.");
    }
}
