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
pub const PROTECTED_DIRS: &[&str] = &[
    "/etc",
    "/bin",
    "/sbin",
    "/usr/bin",
    "/usr/sbin",
    "/lib",
    "/boot",
    "/proc",
    "/var/boos/requests",
    "/var/boos/results",
    "/var/boos/memory",
    "/var/log",
];

// Protected read paths — read-file refuses to read these specific files
// (not directories). For secrets that must never leak via raw file reads.
pub const PROTECTED_READ_PATHS: &[&str] = &[
    "/etc/boos/agent.conf",
    "/etc/boos/gateway_token",
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
        // Requests must also enter through submit so metadata cannot be forged.
        assert!(is_protected_path("/var/boos/requests/req-forged"));
    }

    #[cfg(unix)]
    #[test]
    fn test_nonexistent_write_below_symlinked_protected_parent_is_denied() {
        use std::os::unix::fs::symlink;

        let link = std::env::temp_dir().join(format!(
            "boos-protected-parent-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&link);
        symlink("/etc", &link).unwrap();

        let target = link.join("boos-audit-nonexistent-file");
        assert!(
            is_protected_path(target.to_str().unwrap()),
            "a nonexistent target below a symlink to /etc must remain protected"
        );

        std::fs::remove_file(link).unwrap();
    }
}

/// Check if a path is under a protected directory.
/// Uses lexical comparison first, then resolves symlinks component by
/// component. Resolving only the complete path is insufficient for a new file:
/// its parent may already be a symlink into a protected directory.
pub fn is_protected_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    let lower = normalized.to_lowercase();

    if prefix_matches_protected(&lower) {
        return true;
    }

    let resolved = match resolve_policy_path(&normalized) {
        Some(path) => path.to_lowercase(),
        None => return true,
    };
    prefix_matches_resolved_protected(&resolved)
}

fn prefix_matches_protected(lower: &str) -> bool {
    PROTECTED_DIRS
        .iter()
        .any(|dir| path_has_prefix(lower, &dir.to_lowercase()))
}

fn prefix_matches_resolved_protected(lower: &str) -> bool {
    PROTECTED_DIRS.iter().any(|dir| {
        let resolved_dir = resolve_policy_path(dir)
            .unwrap_or_else(|| normalize_path(dir))
            .to_lowercase();
        path_has_prefix(lower, &resolved_dir)
    })
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    path.starts_with(prefix)
        && (path.len() == prefix.len() || path.as_bytes().get(prefix.len()) == Some(&b'/'))
}

/// Resolve symlink components for policy comparison without requiring the
/// final target to exist. A bounded traversal fails closed on loops.
fn resolve_policy_path(path: &str) -> Option<String> {
    use std::ffi::OsString;
    use std::path::{Component, Path, PathBuf};

    const MAX_SYMLINKS: usize = 40;
    let mut pending = PathBuf::from(normalize_path(path));

    for _ in 0..MAX_SYMLINKS {
        let components: Vec<OsString> = pending
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect();
        let mut current = PathBuf::from("/");
        let mut redirected = false;

        for (index, component) in components.iter().enumerate() {
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    let target = std::fs::read_link(&current).ok()?;
                    let mut next = if target.is_absolute() {
                        target
                    } else {
                        current.parent().unwrap_or(Path::new("/")).join(target)
                    };
                    for remaining in components.iter().skip(index + 1) {
                        next.push(remaining);
                    }
                    pending = PathBuf::from(normalize_path(next.to_string_lossy().as_ref()));
                    redirected = true;
                    break;
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Some(normalize_path(pending.to_string_lossy().as_ref()));
                }
                Err(_) => return None,
            }
        }

        if !redirected {
            return Some(normalize_path(pending.to_string_lossy().as_ref()));
        }
    }

    None
}

/// Check if a path is a protected read path (secrets that must not be read).
pub fn is_protected_read_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    let lower = normalized.to_lowercase();
    if PROTECTED_READ_PATHS
        .iter()
        .any(|protected| lower == protected.to_lowercase())
    {
        return true;
    }

    let resolved = match resolve_policy_path(&normalized) {
        Some(path) => path.to_lowercase(),
        None => return true,
    };
    PROTECTED_READ_PATHS.iter().any(|protected| {
        let resolved_protected = resolve_policy_path(protected)
            .unwrap_or_else(|| normalize_path(protected))
            .to_lowercase();
        resolved == resolved_protected
    })
}

#[test]
fn test_symlink_does_not_crash() {
    use std::os::unix::fs;
    let link = "/tmp/boos-test-symlink-protect";
    let _ = std::fs::remove_file(link);
    // is_protected_path calls canonicalize on existing paths.
    // This test verifies it handles symlinks without panicking.
    fs::symlink("/etc/passwd", link).ok();
    let _result = is_protected_path(link);
    // If canonicalize resolves past the symlink on Linux, it returns true.
    // On macOS /etc→/private/etc, lexically not in PROTECTED_DIRS prefix.
    // Either way, the function must not panic.
    let _ = std::fs::remove_file(link);
}

#[test]
fn test_read_protection_exact_match() {
    assert!(is_protected_read_path("/etc/boos/agent.conf"));
    assert!(is_protected_read_path("/etc/boos/gateway_token"));
    assert!(!is_protected_read_path("/etc/passwd"));
    assert!(!is_protected_read_path("/tmp/test.txt"));
}

#[cfg(unix)]
#[test]
fn test_read_protection_follows_symlink_to_secret() {
    use std::os::unix::fs::symlink;

    let link = std::env::temp_dir().join(format!(
        "boos-protected-read-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&link);
    symlink("/etc/boos/gateway_token", &link).unwrap();

    assert!(
        is_protected_read_path(link.to_str().unwrap()),
        "a symlink must not make the gateway token readable"
    );

    std::fs::remove_file(link).unwrap();
}
