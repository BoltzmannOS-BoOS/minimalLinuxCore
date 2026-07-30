use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const MAX_REQUEST_ID_BYTES: usize = 128;
const TEMP_CREATE_ATTEMPTS: u32 = 10;

pub struct RequestRecord<'a> {
    pub id: &'a str,
    pub requester: &'a str,
    pub command: &'a str,
    pub args: &'a str,
    pub submitted_at: f64,
    pub session_id: Option<&'a str>,
}

impl RequestRecord<'_> {
    pub fn to_kv(&self) -> String {
        let mut content = format!(
            "id={}\nrequester={}\ncommand={}\nargs={}\nsubmitted_at={:.3}\nstatus=pending\n",
            encode_kv_value(self.id),
            encode_kv_value(self.requester),
            encode_kv_value(self.command),
            encode_kv_value(self.args),
            self.submitted_at,
        );
        if let Some(session_id) = self.session_id {
            content.push_str(&format!(
                "session_id={}\n",
                encode_kv_value(session_id)
            ));
        }
        content
    }
}

pub fn publish_request(queue_dir: &Path, record: &RequestRecord<'_>) -> io::Result<PathBuf> {
    validate_request_id(record.id)?;
    let final_path = queue_dir.join(record.id);
    let (temporary_path, mut temporary_file) =
        create_temporary_request(queue_dir, record.id)?;

    let publish_result = (|| {
        temporary_file.write_all(record.to_kv().as_bytes())?;
        temporary_file.sync_data()?;
        fs::hard_link(&temporary_path, &final_path)?;
        Ok(())
    })();
    drop(temporary_file);

    match publish_result {
        Ok(()) => {
            fs::remove_file(&temporary_path)?;
            Ok(final_path)
        }
        Err(error) => {
            match fs::remove_file(&temporary_path) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(io::Error::new(
                    cleanup_error.kind(),
                    format!(
                        "request publish failed: {}; temporary cleanup failed: {}",
                        error, cleanup_error
                    ),
                )),
            }
        }
    }
}

fn create_temporary_request(queue_dir: &Path, id: &str) -> io::Result<(PathBuf, fs::File)> {
    for attempt in 0..TEMP_CREATE_ATTEMPTS {
        let path = queue_dir.join(format!(
            ".{}.{}.{}.tmp",
            id,
            std::process::id(),
            attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "request temporary file namespace is exhausted",
    ))
}

pub(crate) fn validate_request_id(id: &str) -> io::Result<()> {
    let is_valid = id.starts_with("req-")
        && id.len() <= MAX_REQUEST_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if is_valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "request ID must be a req- prefixed ASCII path component",
        ))
    }
}

pub(crate) fn encode_kv_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record<'a>(id: &'a str, requester: &'a str) -> RequestRecord<'a> {
        RequestRecord {
            id,
            requester,
            command: "help",
            args: "",
            submitted_at: 12.5,
            session_id: Some("session-a"),
        }
    }

    #[test]
    fn metadata_newlines_cannot_inject_a_second_command_field() {
        let serialized = record("req-fixed", "agent\ncommand=poweroff").to_kv();
        let command_lines: Vec<&str> = serialized
            .lines()
            .filter(|line| line.starts_with("command="))
            .collect();

        assert_eq!(command_lines, vec!["command=help"]);
        assert!(serialized.contains("requester=agent\\ncommand=poweroff"));
    }

    #[test]
    fn publishing_a_collision_does_not_replace_the_existing_request() {
        let dir = std::env::temp_dir().join(format!(
            "boos-request-publish-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let existing_path = dir.join("req-fixed");
        std::fs::write(&existing_path, "original request").unwrap();

        let error = publish_request(&dir, &record("req-fixed", "agent")).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(existing_path).unwrap(),
            "original request"
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn request_id_cannot_escape_the_queue_directory() {
        let dir = std::env::temp_dir().join(format!(
            "boos-request-id-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let error = publish_request(&dir, &record("../escape", "agent")).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);

        std::fs::remove_dir_all(dir).unwrap();
    }
}
