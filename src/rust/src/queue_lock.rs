use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::raw::c_int;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

const LOCK_EXCLUSIVE: c_int = 2;
const LOCK_NONBLOCKING: c_int = 4;
const LOCK_UNLOCK: c_int = 8;
#[cfg(target_os = "linux")]
const SAFE_OPEN_FLAGS: c_int = 0o400000 | 0o4000; // O_NOFOLLOW | O_NONBLOCK
#[cfg(not(target_os = "linux"))]
const SAFE_OPEN_FLAGS: c_int = 0;

extern "C" {
    fn flock(fd: c_int, operation: c_int) -> c_int;
}

pub struct QueueProcessorLock {
    file: File,
}

impl QueueProcessorLock {
    pub fn acquire(path: &Path) -> io::Result<Option<Self>> {
        let file = open_lock_file(path)?;
        // SAFETY: `file` owns a valid descriptor for the duration of the call.
        let result = unsafe {
            flock(
                file.as_raw_fd(),
                LOCK_EXCLUSIVE | LOCK_NONBLOCKING,
            )
        };
        if result == 0 {
            return Ok(Some(Self { file }));
        }

        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(error)
        }
    }
}

impl Drop for QueueProcessorLock {
    fn drop(&mut self) {
        // SAFETY: the descriptor remains valid until `self.file` is dropped.
        let _ = unsafe { flock(self.file.as_raw_fd(), LOCK_UNLOCK) };
    }
}

fn open_lock_file(path: &Path) -> io::Result<File> {
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "queue lock path must not be a symlink",
        ));
    }

    match OpenOptions::new()
        .read(true)
        .custom_flags(SAFE_OPEN_FLAGS)
        .open(path)
    {
        Ok(file) => require_regular_file(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .custom_flags(SAFE_OPEN_FLAGS)
                .open(path)?;
            let file = require_regular_file(file)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
            Ok(file)
        }
        Err(error) => Err(error),
    }
}

fn require_regular_file(file: File) -> io::Result<File> {
    if file.metadata()?.is_file() {
        Ok(file)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "queue lock path is not a regular file",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_processor_cannot_hold_the_same_queue_lock() {
        let dir = std::env::temp_dir().join(format!(
            "boos-queue-lock-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lock_path = dir.join(".processor.lock");

        let first = QueueProcessorLock::acquire(&lock_path).unwrap();
        let second = QueueProcessorLock::acquire(&lock_path).unwrap();

        assert!(first.is_some());
        assert!(second.is_none(), "two processors must not execute the queue");

        drop(first);
        assert!(QueueProcessorLock::acquire(&lock_path).unwrap().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_symlink_cannot_be_used_as_the_queue_lock_file() {
        use std::os::unix::fs::symlink;

        let dir = std::env::temp_dir().join(format!(
            "boos-queue-lock-symlink-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("attacker-target");
        let lock_path = dir.join(".processor.lock");
        std::fs::write(&target, "must not be opened as the lock").unwrap();
        symlink(&target, &lock_path).unwrap();

        assert!(
            QueueProcessorLock::acquire(&lock_path).is_err(),
            "queue locking must reject a symlink supplied by another principal"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }
}
