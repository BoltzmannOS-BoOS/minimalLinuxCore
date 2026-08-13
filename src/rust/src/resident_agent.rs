use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::thread;
use std::time::Duration;

use crate::config;
use crate::log;
use crate::memory::{self, WorkingMemory};
use crate::memory_namespace::MemoryNamespace;
use crate::principal::{self, PrincipalContext};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const STATUS_TEMP_ATTEMPTS: u32 = 16;

#[derive(Clone, Copy)]
pub struct ResidentState {
    pid: u32,
    started_at: u64,
}

impl ResidentState {
    pub fn ready(pid: u32, started_at: u64) -> Self {
        Self { pid, started_at }
    }
}

struct Resident {
    context: PrincipalContext,
    state: ResidentState,
}

pub fn write_status(context: &PrincipalContext, state: ResidentState) -> io::Result<()> {
    fs::create_dir_all(context.runtime_root())?;
    let status_path = context.status_path();
    let content = format!(
        "principal={}\nstate=ready\npid={}\nstarted_at={}\n",
        context.id().as_str(),
        state.pid,
        state.started_at
    );

    for attempt in 0..STATUS_TEMP_ATTEMPTS {
        let temporary_path = context
            .runtime_root()
            .join(format!(".status.{}.{}.tmp", state.pid, attempt));
        let mut temporary_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };

        let publish_result = (|| {
            temporary_file.write_all(content.as_bytes())?;
            temporary_file.sync_data()?;
            drop(temporary_file);
            fs::rename(&temporary_path, &status_path)
        })();

        return match publish_result {
            Ok(()) => Ok(()),
            Err(error) => match fs::remove_file(&temporary_path) {
                Ok(()) => Err(error),
                Err(cleanup_error) if cleanup_error.kind() == io::ErrorKind::NotFound => {
                    Err(error)
                }
                Err(cleanup_error) => Err(io::Error::new(
                    cleanup_error.kind(),
                    format!(
                        "status publication failed: {}; temporary cleanup failed: {}",
                        error, cleanup_error
                    ),
                )),
            },
        };
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "resident status temporary namespace is exhausted",
    ))
}

fn ensure_memory_session(context: &PrincipalContext, pid: u32, started_at: u64) -> io::Result<()> {
    let namespace = MemoryNamespace::from_context(context);
    match WorkingMemory::load_from(&namespace) {
        Ok(memory) if !memory.session_id.is_empty() => Ok(()),
        Ok(_) => WorkingMemory::new(format!("resident-{started_at}-{pid}")).save_in(&namespace),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            WorkingMemory::new(format!("resident-{started_at}-{pid}")).save_in(&namespace)
        }
        Err(error) => Err(error),
    }
}

fn initialize_resident<F>(
    resolve_context: F,
    pid: u32,
    started_at: u64,
) -> io::Result<Resident>
where
    F: FnOnce() -> io::Result<PrincipalContext>,
{
    let context = resolve_context()?;
    ensure_memory_session(&context, pid, started_at)?;
    let state = ResidentState::ready(pid, started_at);
    write_status(&context, state)?;
    Ok(Resident { context, state })
}

pub fn run() -> i32 {
    let pid = std::process::id();
    let started_at = memory::now_secs();
    let resident = match initialize_resident(principal::current_context, pid, started_at) {
        Ok(resident) => resident,
        Err(error) => {
            let escaped_error = log::json_escape(&error.to_string());
            log::log(
                "boos-agent",
                "error",
                &[("op", "resident_start"), ("error", &escaped_error)],
            );
            eprintln!("Cannot start resident principal: {}", error);
            return config::EXIT_ERROR;
        }
    };

    log::log(
        "boos-agent",
        "resident_ready",
        &[
            ("principal", resident.context.id().as_str()),
            ("pid", &pid.to_string()),
        ],
    );
    println!(
        "resident_ready principal={} pid={}",
        resident.context.id().as_str(),
        pid
    );

    loop {
        thread::sleep(HEARTBEAT_INTERVAL);
        if let Err(error) = write_status(&resident.context, resident.state) {
            let escaped_error = log::json_escape(&error.to_string());
            log::log(
                "boos-agent",
                "error",
                &[("op", "resident_heartbeat"), ("error", &escaped_error)],
            );
            eprintln!("Resident heartbeat failed: {}", error);
            return config::EXIT_ERROR;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::principal::{
        configured_context, PrincipalDefinition, PrincipalId,
    };
    use std::io;
    use std::os::unix::fs::symlink;

    fn temporary_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "boos-resident-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn context(root: &std::path::Path) -> crate::principal::PrincipalContext {
        let definition = PrincipalDefinition {
            id: PrincipalId::parse("resident").unwrap(),
            user: "boos-agent".to_string(),
            uid: 101,
            gid: 101,
            enabled: true,
        };
        configured_context(&definition, root)
    }

    #[test]
    fn initialization_prepares_memory_before_publishing_ready_status() {
        let root = temporary_root("ready");
        let context = context(&root);

        let resident = initialize_resident(|| Ok(context.clone()), 42, 100).unwrap();

        assert_eq!(resident.context.id().as_str(), "resident");
        assert!(context.memory_root().join("working.kv").is_file());
        assert_eq!(
            std::fs::read_to_string(context.status_path()).unwrap(),
            "principal=resident\nstate=ready\npid=42\nstarted_at=100\n"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn status_publication_replaces_a_symlink_without_following_it() {
        let root = temporary_root("symlink");
        let context = context(&root);
        std::fs::create_dir_all(context.runtime_root()).unwrap();
        let outside = root.join("outside");
        std::fs::write(&outside, "untouched").unwrap();
        symlink(&outside, context.status_path()).unwrap();

        write_status(&context, ResidentState::ready(42, 100)).unwrap();

        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "untouched");
        assert_eq!(
            std::fs::read_to_string(context.status_path()).unwrap(),
            "principal=resident\nstate=ready\npid=42\nstarted_at=100\n"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolution_failure_cannot_publish_readiness() {
        let result = initialize_resident(
            || Err(io::Error::new(io::ErrorKind::PermissionDenied, "invalid principal")),
            42,
            100,
        );

        assert_eq!(
            result.err().unwrap().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn memory_failure_cannot_publish_readiness() {
        let root = temporary_root("memory-error");
        let context = context(&root);
        std::fs::create_dir_all(context.memory_root()).unwrap();
        std::fs::create_dir(context.memory_root().join("working.kv")).unwrap();

        let result = initialize_resident(|| Ok(context.clone()), 42, 100);

        assert!(result.is_err());
        assert!(!context.status_path().exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
