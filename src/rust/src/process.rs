use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::bounded_process;
use crate::config;
use crate::log;
use crate::principal::{self, PrincipalContext};
use crate::queue_lock::QueueProcessorLock;
use crate::queue_record;
use crate::request_publish::encode_kv_value;

/// Execute a command and capture its output, enforcing MAX_OUTPUT_BYTES limit.
/// Returns (stdout, exit_code, was_truncated).
///
/// Reads stdout and stderr concurrently via threads to avoid pipe-buffer deadlock:
/// if the child fills stderr before stdout, sequential read would block forever.
fn capture_output(
    context: &PrincipalContext,
    cmd: &str,
    args: &[&str],
) -> (String, i32, bool) {
    let mut command = process::Command::new(cmd);
    command
        .args(args)
        .env("BOOS_PRINCIPAL_ID", context.id().as_str())
        .gid(context.gid())
        .uid(context.uid());
    let captured = match bounded_process::run_with_limits(
        &mut command,
        config::MAX_OUTPUT_BYTES,
        Duration::from_secs(config::QUEUE_CHILD_TIMEOUT_SECS),
    ) {
        Ok(captured) => captured,
        Err(error) => {
            return (
                format!("Failed to execute child process: {}", error),
                config::EXIT_ERROR,
                false,
            );
        }
    };
    let truncated = captured.stdout_truncated || captured.stderr_truncated;
    let exit_code = if captured.timed_out {
        config::EXIT_ERROR
    } else {
        captured.status.code().unwrap_or(config::EXIT_ERROR)
    };

    // Combine stdout and stderr
    let mut output = String::from_utf8_lossy(&captured.stdout).to_string();
    let err_out = String::from_utf8_lossy(&captured.stderr);
    if !err_out.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&err_out);
    }

    if truncated {
        output.push_str(&format!(
            "\n[stdout or stderr truncated at {} bytes]",
            config::MAX_OUTPUT_BYTES
        ));
    }
    if captured.timed_out {
        output.push_str(&format!(
            "\n[terminated after {} seconds]",
            config::QUEUE_CHILD_TIMEOUT_SECS
        ));
    }

    (output, exit_code, truncated)
}

fn build_result_metadata(
    owned_request: &queue_record::OwnedQueuedRequest,
    verdict: &str,
    exit_code: i32,
    started_at: f64,
    finished_at: f64,
    duration_ms: u128,
) -> String {
    let request = &owned_request.request;
    let trace_requester = request
        .claimed_requester
        .as_deref()
        .unwrap_or("unknown");
    let mut metadata = format!(
        "id={}\nprincipal={}\nrequester={}\ncommand={}\nargs={}\nverdict={}\nexit_code={}\nstarted_at={:.3}\nfinished_at={:.3}\nduration_ms={}\n",
        encode_kv_value(&request.id),
        encode_kv_value(owned_request.principal.as_str()),
        encode_kv_value(trace_requester),
        encode_kv_value(&request.command),
        encode_kv_value(&request.args),
        verdict,
        exit_code,
        started_at,
        finished_at,
        duration_ms
    );
    if let Some(session_id) = &request.session_id {
        metadata.push_str(&format!(
            "session_id={}\n",
            encode_kv_value(session_id)
        ));
    }
    metadata
}

/// Scan one principal's runtime root for files modified since the marker.
fn files_changed_since(root: &Path, marker_ts: f64) -> String {
    let mut touched = Vec::new();
    let _ = walk_dir(root, &mut touched, marker_ts, 0);
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

pub fn process_principal_queue(context: &PrincipalContext) -> io::Result<u32> {
    let request_directory = context.requests_dir();
    let result_directory = context.results_dir();
    for directory in [&request_directory, &result_directory] {
        fs::create_dir_all(directory)?;
    }

    let lock_path = request_directory.join(".processor.lock");
    let _queue_lock = match QueueProcessorLock::acquire(&lock_path) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            return Ok(0);
        }
        Err(error) => {
            log::log(
                "boos-process",
                "queue_lock_error",
                &[
                    ("principal", context.id().as_str()),
                    ("error", &error.to_string()),
                ],
            );
            return Err(error);
        }
    };

    let trace = log::get_trace_level();
    let mut processed = 0u32;

    let dir = fs::read_dir(&request_directory)?;

    let mut entries: Vec<_> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("req-"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let owned_request = match queue_record::load_request(&path) {
            Ok(request) => request.with_principal(context.id().clone()),
            Err(error) => {
                log::log(
                    "boos-process",
                    "invalid_request",
                    &[
                        ("file", &entry.file_name().to_string_lossy()),
                        ("error", &error.to_string()),
                    ],
                );
                if matches!(
                    error.kind(),
                    io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput
                ) {
                    if let Err(remove_error) = fs::remove_file(&path) {
                        log::log(
                            "boos-process",
                            "request_remove_error",
                            &[("error", &remove_error.to_string())],
                        );
                    }
                }
                continue;
            }
        };
        let request = &owned_request.request;
        let id = request.id.as_str();
        let cmd = request.command.as_str();
        let args = request.args.as_str();
        let requester = request
            .claimed_requester
            .as_deref()
            .unwrap_or("unknown");

        match queue_record::existing_result(&result_directory, id) {
            Ok(true) => {
                log::log(
                    "boos-process",
                    "request_result_already_exists",
                    &[("id", id)],
                );
                if let Err(error) = fs::remove_file(&path) {
                    log::log(
                        "boos-process",
                        "request_remove_error",
                        &[("id", id), ("error", &error.to_string())],
                    );
                }
                continue;
            }
            Ok(false) => {}
            Err(error) => {
                log::log(
                    "boos-process",
                    "result_preflight_error",
                    &[("id", id), ("error", &error.to_string())],
                );
                continue;
            }
        }

        let started_at = log::uptime_secs();
        let last_command_path = context.runtime_root().join("last-command");
        let prev_cmd = fs::read_to_string(&last_command_path).unwrap_or_default();
        let prev_cmd = prev_cmd.trim();

        // Verbose: record a baseline timestamp so we can scan this principal's
        // runtime root. ext2 stores integer-second mtimes, so we
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
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"executing\",\"id\":\"{}\",\"principal\":\"{}\",\"requester\":\"{}\",\"command\":\"{}\",\"args\":\"{}\",\"prev\":\"{}\"}}",
                started_at, log::json_escape(id),
                log::json_escape(context.id().as_str()), log::json_escape(requester),
                log::json_escape(cmd), log::json_escape(args), log::json_escape(prev_cmd)
            ));
        } else {
            log::append_log_line(&format!(
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"executing\",\"id\":\"{}\",\"principal\":\"{}\",\"requester\":\"{}\",\"command\":\"{}\",\"args\":\"{}\"}}",
                started_at, log::json_escape(id),
                log::json_escape(context.id().as_str()), log::json_escape(requester),
                log::json_escape(cmd), log::json_escape(args)
            ));
        }

        // Pass cmd as first arg, remaining args split by whitespace.
        // Avoids: format!("{} {}", cmd, args) → split_whitespace (Bug: destroys
        // spacing if args were originally multi-word).
        let mut exec_vec = vec![cmd];
        exec_vec.extend(args.split_whitespace());
        let (output, exit_code, _truncated) =
            capture_output(context, "/bin/boos-exec", &exec_vec);

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
            let found = files_changed_since(context.runtime_root(), marker_ts);
            if !found.is_empty() {
                log::append_log_line(&format!(
                    "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"fs_trace\",\"principal\":\"{}\",\"files\":\"{}\"}}",
                    log::uptime_secs(), log::json_escape(context.id().as_str()),
                    log::json_escape(&found)
                ));
            }
            found
        } else {
            String::new()
        };

        // Update last-cmd
        let last = format!("{} {}", cmd, args);
        let _ = fs::write(&last_command_path, last.trim());

        let mut result_content = build_result_metadata(
            &owned_request,
            verdict,
            exit_code,
            started_at,
            finished_at,
            duration.into(),
        );

        if !prev_cmd.is_empty() {
            result_content.push_str(&format!(
                "prev_command={}\n",
                encode_kv_value(prev_cmd)
            ));
        }
        if !files_touched.is_empty() {
            result_content.push_str(&format!(
                "files_touched={}\n",
                encode_kv_value(&files_touched)
            ));
        }

        result_content.push_str("---\n");
        result_content.push_str(&output);
        result_content.push('\n');

        if let Err(error) = queue_record::publish_result(
            &result_directory,
            id,
            result_content.as_bytes(),
        ) {
            log::log(
                "boos-process",
                "result_publish_error",
                &[("id", id), ("error", &error.to_string())],
            );
            eprintln!("Cannot publish result for {}: {}", id, error);
            processed += 1;
            continue;
        }

        // Log completion
        if trace == log::TraceLevel::Verbose {
            log::append_log_line(&format!(
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"completed\",\"id\":\"{}\",\"verdict\":\"{}\",\"exit_code\":{},\"duration_ms\":{},\"files\":\"{}\"}}",
                log::uptime_secs(), log::json_escape(id), verdict, exit_code, duration,
                log::json_escape(&files_touched)
            ));
        } else {
            log::append_log_line(&format!(
                "{{\"ts\":{:.3},\"component\":\"boos-process\",\"event\":\"completed\",\"id\":\"{}\",\"verdict\":\"{}\",\"exit_code\":{},\"duration_ms\":{}}}",
                log::uptime_secs(), log::json_escape(id), verdict, exit_code, duration
            ));
        }

        println!("[{}] {} ({}ms)", id, verdict, duration);
        println!("{}", output);

        // A durable result is the completion marker. If request deletion
        // fails, the next processor observes that marker and only retries the
        // cleanup instead of executing the command again.
        if let Err(error) = fs::remove_file(&path) {
            log::log(
                "boos-process",
                "request_remove_error",
                &[("id", id), ("error", &error.to_string())],
            );
        }
        processed += 1;
    }

    Ok(processed)
}

pub fn main() {
    let definitions = match principal::load_definitions() {
        Ok(definitions) => definitions,
        Err(error) => {
            eprintln!("Cannot load BoOS principals: {}", error);
            return;
        }
    };
    let mut processed = 0u32;
    for definition in definitions.iter().filter(|definition| definition.enabled) {
        let context = principal::configured_context(
            definition,
            Path::new(config::PRINCIPAL_RUNTIME_DIR),
        );
        match process_principal_queue(&context) {
            Ok(count) => processed += count,
            Err(error) => {
                eprintln!(
                    "Cannot process principal {}: {}",
                    context.id().as_str(),
                    error
                );
                log::log(
                    "boos-process",
                    "principal_queue_error",
                    &[
                        ("principal", context.id().as_str()),
                        ("error", &error.to_string()),
                    ],
                );
            }
        }
    }
    if processed == 0 {
        println!("No pending requests.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::principal::{
        resolve_context, PrincipalDefinition, PrincipalId,
    };
    use crate::queue_record::{OwnedQueuedRequest, QueuedRequest};
    use std::path::Path;

    fn principal_context(runtime_root: &Path) -> crate::principal::PrincipalContext {
        let definition = PrincipalDefinition {
            id: PrincipalId::parse("resident").unwrap(),
            user: "boos-agent".to_string(),
            uid: 101,
            gid: 101,
            enabled: true,
        };
        resolve_context(&[definition], "resident", 101, runtime_root).unwrap()
    }

    #[test]
    fn empty_principal_queue_prepares_only_its_own_spool() {
        let runtime_root = std::env::temp_dir().join(format!(
            "boos-process-principal-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&runtime_root);
        let context = principal_context(&runtime_root);

        let processed = process_principal_queue(&context).unwrap();

        assert_eq!(processed, 0);
        assert!(context.requests_dir().is_dir());
        assert!(context.results_dir().is_dir());
        assert!(!runtime_root.join("requests").exists());
        std::fs::remove_dir_all(runtime_root).unwrap();
    }

    #[test]
    fn principal_child_drops_root_supplementary_groups() {
        let current_uid = std::process::Command::new("id")
            .arg("-u")
            .output()
            .unwrap();
        if String::from_utf8_lossy(&current_uid.stdout).trim() != "0" {
            return;
        }

        let context = principal_context(Path::new("/runtime"));
        let (uid, uid_exit, _) = capture_output(&context, "/usr/bin/id", &["-u"]);
        let (gid, gid_exit, _) = capture_output(&context, "/usr/bin/id", &["-g"]);
        let (groups, groups_exit, _) = capture_output(&context, "/usr/bin/id", &["-G"]);

        assert_eq!((uid_exit, gid_exit, groups_exit), (0, 0, 0));
        assert_eq!(uid.trim(), "101");
        assert_eq!(gid.trim(), "101");
        assert!(
            !groups.split_whitespace().any(|group| group == "0"),
            "root supplementary group leaked into principal child: {groups:?}"
        );
    }

    #[test]
    fn result_metadata_uses_spool_principal_as_owner() {
        let owned = OwnedQueuedRequest {
            principal: PrincipalId::parse("resident").unwrap(),
            request: QueuedRequest {
                id: "req-owned".to_string(),
                claimed_requester: Some("forged".to_string()),
                command: "help".to_string(),
                args: String::new(),
                session_id: Some("session-a".to_string()),
            },
        };

        let metadata =
            build_result_metadata(&owned, "allowed", 0, 1.0, 2.0, 1000);

        assert!(metadata.contains("principal=resident\n"));
        assert!(metadata.contains("requester=forged\n"));
        assert!(!metadata.contains("principal=forged"));
    }
}
