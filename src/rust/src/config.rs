pub const LOG_FILE: &str = "/var/log/boos.log";
pub const CAP_FILE: &str = "/etc/boos/capabilities.conf";
pub const CMD_DIR: &str = "/etc/boos/commands";
pub const DEBUG_CONF: &str = "/etc/boos/debug.conf";
pub const REQ_DIR: &str = "/var/boos/requests";
pub const RESULT_DIR: &str = "/var/boos/results";
pub const LAST_CMD_FILE: &str = "/var/boos/last-cmd";
pub const UPTIME_FILE: &str = "/proc/uptime";
pub const DAEMON_DIR: &str = "/etc/boos/daemons";
pub const DAEMON_RUN_DIR: &str = "/var/boos/daemons";

pub const MAX_OUTPUT_BYTES: usize = 1_048_576; // 1MB
pub const MAX_LOG_LINE_LEN: usize = 4096;
pub const GATEWAY_DEFAULT_PORT: u16 = 5555;
pub const QUEUE_POLL_MS: u64 = 200;
