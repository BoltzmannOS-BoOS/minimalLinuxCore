use std::fs::OpenOptions;
use std::io::Write;

use crate::config;

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
        // Only log denials, errors, unknown
        if event != "denied" && event != "error" && event != "unknown" && event != "config" {
            return;
        }
    }

    let ts = uptime_secs();
    let mut line = format!("{{\"ts\":{:.3},\"component\":\"{}\",\"event\":\"{}\"", ts, component, event);

    for (k, v) in fields {
        // The caller already JSON-escapes v
        line.push_str(&format!(",\"{}\":\"{}\"", k, v));
    }
    line.push('}');
    line.push('\n');

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(config::LOG_FILE) {
        let _ = f.write_all(line.as_bytes());
    }
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

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(config::LOG_FILE) {
        let _ = f.write_all(line.as_bytes());
    }
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
/// Includes the newline in the same write buffer to prevent interleaving.
pub fn append_log_line(line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(config::LOG_FILE) {
        // Single write: line + newline in one buffer to prevent interleaving
        let mut buf = line.as_bytes().to_vec();
        buf.push(b'\n');
        let _ = f.write_all(&buf);
    }
}

/// Format a timestamp as "HH:MM:SS.mmm" from uptime seconds (naive, just format the float).
pub fn fmt_ts(ts: f64) -> String {
    let total_secs = ts as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    let ms = ((ts - ts.floor()) * 1000.0) as u64;
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, ms)
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

    #[test]
    fn test_fmt_ts() {
        let ts = 3661.500; // 1h 1m 1s 500ms
        let formatted = fmt_ts(ts);
        assert_eq!(formatted, "01:01:01.500");
    }

    #[test]
    fn test_fmt_ts_zero() {
        assert_eq!(fmt_ts(0.0), "00:00:00.000");
    }
}
