pub const LOG_FILE: &str = "/var/log/boos.log";
pub const CAP_FILE: &str = "/etc/boos/capabilities.conf";
pub const CMD_DIR: &str = "/etc/boos/commands";
pub const DEBUG_CONF: &str = "/etc/boos/debug.conf";
pub const REQ_DIR: &str = "/var/boos/requests";
pub const RESULT_DIR: &str = "/var/boos/results";
pub const LAST_CMD_FILE: &str = "/var/boos/last-cmd";
pub const UPTIME_FILE: &str = "/proc/uptime";

pub const MAX_OUTPUT_BYTES: usize = 1_048_576; // 1MB
// Enforced by log::append_log_line as a hard cap on per-line size so a
// runaway component can't fill the disk with one giant entry.
pub const MAX_LOG_LINE_LEN: usize = 4096;
pub const GATEWAY_DEFAULT_PORT: u16 = 5555;

// Exit code contract for boos-exec. process.rs translates these to verdicts.
// External programs invoked via `exec=/path/to/bin` may produce arbitrary
// codes; process.rs maps anything outside this set to "error".
pub const EXIT_ALLOWED: i32 = 0;
pub const EXIT_DENIED: i32 = 1;
pub const EXIT_ERROR: i32 = 2;
pub const EXIT_UNKNOWN: i32 = 3;

// Log rotation: when the log file reaches this size, it is rotated to
// .1 (and the previous .1 to .2). Set high enough that boot-time logs
// don't trigger churn.
pub const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_LOG_BACKUPS: u32 = 2;

// Gateway concurrency cap. Each in-flight request runs in its own thread;
// further connections are accepted but immediately rejected with a message.
// On a single-user local QEMU port-forward this is more than enough.
pub const MAX_GATEWAY_THREADS: usize = 64;

// Agent memory system paths
pub const MEMORY_DIR: &str = "/var/boos/memory";

// BIOS: hardcoded boundaries. Cannot be overridden by any file.
// Only actions that cause irreversible damage belong here.
pub const IMMUTABLE_DENY: &[&str] = &[
    "allow_reset",       // clearing all state is irreversible
    "allow_net_write",   // agent cannot exfiltrate data via network
    "allow_proc_kill_system", // agent cannot kill gateway/supervisor/init
];

// Protected paths — write-file refuses to write to these directories.
// The agent can READ but cannot WRITE to these via raw file operations.
// System directories go through boos-exec builtins (submit, remember, etc.)
pub const PROTECTED_DIRS: &[&str] = &[
    "/etc",
    "/bin",
    "/sbin",
    "/usr/bin",
    "/usr/sbin",
    "/lib",
    "/boot",
    "/proc",
    "/var/boos/results",   // must use submit pipeline
    "/var/boos/memory",    // must use remember/observe/recall
    "/var/log",            // must use boos-exec logging
];

// Exec allowlist — check full command prefix, not just binary name.
// "cargo build" matches "cargo build --release" but NOT "cargo run".
pub const EXEC_ALLOWLIST: &[&str] = &[
    "cargo build",
    "cargo test",
    "cargo --version",
];

/// Normalize a path for security comparison: resolve .., collapse //, lowercase.
/// Does NOT access the filesystem — purely lexical normalization.
pub fn normalize_path(path: &str) -> String {
    let mut components: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => continue,  // skip empty (from //) and .
            ".." => { let _ = components.pop(); }
            _ => components.push(part),
        }
    }
    if components.is_empty() {
        return "/".to_string();
    }
    let mut result = String::from("/");
    for (i, c) in components.iter().enumerate() {
        if i > 0 { result.push('/'); }
        result.push_str(c);
    }
    result
}


// ── Body: Homeostasis thresholds ──────────────────────────────────────────
pub const HEALTH_MEMORY_RECENT_MAX: usize = 80;   // recent entries before warning
pub const HEALTH_UPTIME_WARN: u64 = 3600;         // 1 hour — time to reflect
#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn test_normalize_double_slash() {
        assert_eq!(normalize_path("//etc/passwd"), "/etc/passwd");
    }
    #[test]
    fn test_normalize_dotdot() {
        assert_eq!(normalize_path("/tmp/../../etc/passwd"), "/etc/passwd");
    }
    #[test]
    fn test_normalize_dot() {
        assert_eq!(normalize_path("/./etc/./passwd"), "/etc/passwd");
    }
    #[test]
    fn test_normalize_mixed() {
        assert_eq!(normalize_path("/tmp/../tmp/./../etc/passwd"), "/etc/passwd");
    }
    #[test]
    fn test_is_protected_etc() {
        assert!(is_protected_path("/etc/passwd"));
    }
    #[test]
    fn test_is_protected_etc_traversal() {
        assert!(is_protected_path("/tmp/../../etc/passwd"));
    }
    #[test]
    fn test_is_protected_double_slash() {
        assert!(is_protected_path("//etc/passwd"));
    }
    #[test]
    fn test_is_protected_uppercase() {
        assert!(is_protected_path("/ETC/passwd"));
    }
    #[test]
    fn test_is_protected_mixed_case() {
        assert!(is_protected_path("/eTc/PaSsWd"));
    }
    #[test]
    fn test_is_not_protected_tmp() {
        assert!(!is_protected_path("/tmp/safe.txt"));
    }
    #[test]
    fn test_is_not_protected_var() {
        // /var itself is NOT protected — agent can create /var/scripts etc
        assert!(!is_protected_path("/var/scripts/myscript.sh"));
        // But /var/boos/results IS protected (must use submit pipeline)
        assert!(is_protected_path("/var/boos/results/req-1.out"));
    }
}

/// Check if a path is under a protected directory.
/// Uses normalize_path for safe comparison.
pub fn is_protected_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    let lower = normalized.to_lowercase();
    for dir in PROTECTED_DIRS {
        let dirl = dir.to_lowercase();
        if lower.starts_with(&dirl) && (lower.len() == dirl.len() || lower.as_bytes()[dirl.len()] == b'/') {
            return true;
        }
    }
    false
}
