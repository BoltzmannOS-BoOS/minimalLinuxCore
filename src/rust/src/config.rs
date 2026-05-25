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
