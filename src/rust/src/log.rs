use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config;

/// Rotate the log file when it grows past `MAX_LOG_BYTES`. The rotation
/// shifts `boos.log` → `boos.log.1` → `boos.log.2`, dropping the oldest.
/// Called opportunistically from `write_log_bytes` after each write.
///
/// Errors are intentionally swallowed: the log path is best-effort, and if
/// rotation fails we'd rather keep appending than panic out.
fn maybe_rotate_log(current_size: u64) {
    if current_size < config::MAX_LOG_BYTES {
        return;
    }
    // Shift older backups down: .1 → .2, etc.
    for i in (1..config::MAX_LOG_BACKUPS).rev() {
        let from = format!("{}.{}", config::LOG_FILE, i);
        let to = format!("{}.{}", config::LOG_FILE, i + 1);
        let _ = fs::rename(&from, &to);
    }
    let _ = fs::rename(config::LOG_FILE, format!("{}.1", config::LOG_FILE));
}

/// Cheap counter to amortise stat() calls. We only check rotation every
/// LOG_ROTATE_CHECK_EVERY writes — most writes do an append-and-return.
static LOG_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);
const LOG_ROTATE_CHECK_EVERY: u64 = 64;

/// Append a byte slice to the log file, enforcing per-line size and
/// triggering rotation on overflow. All callers below funnel through here.
fn write_log_bytes(line: &[u8]) {
    // Per-line cap: truncate runaway lines so a single bad entry can't fill
    // the disk. We keep the newline if present.
    let mut buf: Vec<u8>;
    let payload = if line.len() > config::MAX_LOG_LINE_LEN {
        buf = line[..config::MAX_LOG_LINE_LEN - 1].to_vec();
        buf.push(b'\n');
        &buf
    } else {
        line
    };

    let mut f = match OpenOptions::new().create(true).append(true).open(config::LOG_FILE) {
        Ok(f) => f,
        Err(_) => return,
    };
    let _ = f.write_all(payload);

    let n = LOG_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    if n % LOG_ROTATE_CHECK_EVERY == 0 {
        if let Ok(meta) = f.metadata() {
            maybe_rotate_log(meta.len());
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum TraceLevel {
    Quiet,
    Normal,
    Verbose,
}

/// Read trace level from config file.
pub fn get_trace_level() -> TraceLevel {
    match std::fs::read_to_string(config::DEBUG_CONF) {
        Ok(s) => {
            for line in s.lines() {
                if let Some(val) = line.strip_prefix("trace_level=") {
                    return match val.trim() {
                        "quiet" => TraceLevel::Quiet,
                        "verbose" => TraceLevel::Verbose,
                        _ => TraceLevel::Normal,
                    };
                }
            }
            TraceLevel::Normal
        }
        Err(_) => TraceLevel::Normal,
    }
}

/// Read /proc/uptime as fractional seconds.
pub fn uptime_secs() -> f64 {
    std::fs::read_to_string(config::UPTIME_FILE)
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|w| w.parse::<f64>().ok()))
        .flatten()
        .unwrap_or(0.0)
}

/// Escape a string for JSON value: handle `"`, `\`, and control characters.
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Write a JSON log line. In Quiet mode, only denials/errors are logged.
/// The `fields` are appended as `"key":"value"` pairs (already JSON-escaped values).
pub fn log_event(component: &str, event: &str, fields: &[(&str, &str)]) {
    let trace = get_trace_level();

    if trace == TraceLevel::Quiet {
        if event != "denied" && event != "error" && event != "unknown" && event != "config" {
            return;
        }
    }

    let ts = uptime_secs();
    let mut line = format!("{{\"ts\":{:.3},\"component\":\"{}\",\"event\":\"{}\"", ts, component, event);

    for (k, v) in fields {
        line.push_str(&format!(",\"{}\":\"{}\"", k, v));
    }
    line.push('}');
    line.push('\n');

    write_log_bytes(line.as_bytes());
}

/// Log a permitted command execution. Respects trace level and includes prev_command in verbose.
pub fn log_allowed(command: &str, desc: &str) {
    let trace = get_trace_level();
    let component = "boos-exec";
    let ts = uptime_secs();

    if trace == TraceLevel::Quiet {
        return; // allowed events are not logged in quiet mode
    }

    let prev = if trace == TraceLevel::Verbose {
        std::fs::read_to_string(config::LAST_CMD_FILE).unwrap_or_default()
    } else {
        String::new()
    };

    let prev = prev.trim();
    let mut line = format!(
        "{{\"ts\":{:.3},\"component\":\"{}\",\"event\":\"allowed\",\"command\":\"{}\",\"desc\":\"{}\"",
        ts, component, json_escape(command), json_escape(desc)
    );

    if trace == TraceLevel::Verbose && !prev.is_empty() {
        line.push_str(&format!(",\"prev\":\"{}\"", json_escape(prev)));
    }
    line.push('}');
    line.push('\n');

    write_log_bytes(line.as_bytes());
}

/// Log a denied command.
pub fn log_denied(command: &str) {
    log_event("boos-exec", "denied", &[("command", &json_escape(command))]);
}

/// Log an unknown command.
pub fn log_unknown(command: &str) {
    log_event("boos-exec", "unknown", &[("command", &json_escape(command))]);
}

/// Generic event logger for other components.
pub fn log(component: &str, event: &str, fields: &[(&str, &str)]) {
    log_event(component, event, fields);
}

/// Append an arbitrary line to the operation log (used by process/submit/gateway).
/// The newline is appended atomically so multi-process writers don't interleave.
pub fn append_log_line(line: &str) {
    let mut buf = line.as_bytes().to_vec();
    buf.push(b'\n');
    write_log_bytes(&buf);
}

/// Compute duration in milliseconds between two uptime timestamps.
pub fn duration_ms(start: f64, end: f64) -> u64 {
    ((end - start) * 1000.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_escape_normal_string() {
        assert_eq!(json_escape("hello"), "hello");
        assert_eq!(json_escape("hello world"), "hello world");
    }

    #[test]
    fn test_json_escape_quotes() {
        assert_eq!(json_escape("he\"llo"), "he\\\"llo");
    }

    #[test]
    fn test_json_escape_backslash() {
        assert_eq!(json_escape("path\\file"), "path\\\\file");
    }

    #[test]
    fn test_json_escape_newline() {
        assert_eq!(json_escape("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_json_escape_control_chars() {
        assert_eq!(json_escape("\x00"), "\\u0000");
        assert_eq!(json_escape("\x1b"), "\\u001b");
    }

    #[test]
    fn test_json_escape_combined() {
        let input = "say \"hello\"\n\tworld";
        let escaped = json_escape(input);
        assert!(escaped.contains("\\\""));
        assert!(escaped.contains("\\n"));
        assert!(escaped.contains("\\t"));
        // Should not contain literal newline or tab
        assert!(!escaped.contains('\n'));
        assert!(!escaped.contains('\t'));
    }

    #[test]
    fn test_duration_ms() {
        assert_eq!(duration_ms(0.0, 1.0), 1000);
        assert_eq!(duration_ms(10.0, 10.5), 500);
        assert_eq!(duration_ms(100.0, 100.001), 1);
    }

    #[test]
    fn test_duration_ms_zero() {
        assert_eq!(duration_ms(5.0, 5.0), 0);
    }
}
