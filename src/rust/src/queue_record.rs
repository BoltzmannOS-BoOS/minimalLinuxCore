use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::config;
use crate::principal::PrincipalId;
use crate::request_publish::validate_request_id;

const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_RESULT_BYTES: u64 = (config::MAX_OUTPUT_BYTES as u64) * 8;
const TEMP_CREATE_ATTEMPTS: u32 = 10;
#[cfg(target_os = "linux")]
const SAFE_READ_FLAGS: i32 = 0o400000 | 0o4000; // O_NOFOLLOW | O_NONBLOCK
#[cfg(not(target_os = "linux"))]
const SAFE_READ_FLAGS: i32 = 0;

pub struct QueuedRequest {
    pub id: String,
    pub claimed_requester: Option<String>,
    pub command: String,
    pub args: String,
    pub session_id: Option<String>,
}

pub struct OwnedQueuedRequest {
    pub principal: PrincipalId,
    pub request: QueuedRequest,
}

impl QueuedRequest {
    pub fn with_principal(self, principal: PrincipalId) -> OwnedQueuedRequest {
        OwnedQueuedRequest {
            principal,
            request: self,
        }
    }
}

pub fn load_request(path: &Path) -> io::Result<QueuedRequest> {
    reject_symlink(path)?;
    let filename = path.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
        invalid_record("request filename is not valid UTF-8")
    })?;
    validate_request_id(filename)?;

    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(SAFE_READ_FLAGS)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_REQUEST_BYTES {
        return Err(invalid_record(
            "request must be a bounded regular file",
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(invalid_record("request exceeds its size limit"));
    }
    let content = String::from_utf8(bytes)
        .map_err(|_| invalid_record("request is not valid UTF-8"))?;
    let fields = parse_unique_fields(&content)?;

    let id = required_field(&fields, "id")?;
    if id != filename {
        return Err(invalid_record(
            "request body ID does not match its queue filename",
        ));
    }
    let command = required_field(&fields, "command")?;
    let status = required_field(&fields, "status")?;
    if status != "pending" {
        return Err(invalid_record("request status must be pending"));
    }

    let session_id = fields.get("session_id").cloned();
    if session_id
        .as_deref()
        .is_some_and(|value| !config::is_valid_runtime_id(value))
    {
        return Err(invalid_record("request session ID is invalid"));
    }

    Ok(QueuedRequest {
        id: id.to_string(),
        claimed_requester: fields.get("requester").cloned(),
        command: command.to_string(),
        args: fields.get("args").cloned().unwrap_or_default(),
        session_id,
    })
}

pub fn existing_result(results_dir: &Path, request_id: &str) -> io::Result<bool> {
    validate_request_id(request_id)?;
    let path = results_dir.join(format!("{}.out", request_id));
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(SAFE_READ_FLAGS)
                .open(&path)?;
            if file.metadata()?.len() > MAX_RESULT_BYTES {
                return Err(invalid_record("existing result exceeds its size limit"));
            }
            let mut content = String::new();
            Read::by_ref(&mut file)
                .take(MAX_RESULT_BYTES + 1)
                .read_to_string(&mut content)?;
            if content.len() as u64 > MAX_RESULT_BYTES {
                return Err(invalid_record("existing result exceeds its size limit"));
            }
            validate_existing_result(&content, request_id)?;
            Ok(true)
        }
        Ok(_) => Err(invalid_record(
            "existing result path is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn validate_existing_result(content: &str, request_id: &str) -> io::Result<()> {
    let (metadata, _) = content
        .split_once("\n---\n")
        .ok_or_else(|| invalid_record("existing result has no output delimiter"))?;
    let fields = parse_unique_fields(metadata)?;
    if required_field(&fields, "id")? != request_id {
        return Err(invalid_record(
            "existing result ID does not match its filename",
        ));
    }
    required_field(&fields, "exit_code")?
        .parse::<i32>()
        .map_err(|_| invalid_record("existing result exit code is invalid"))?;
    Ok(())
}

pub fn publish_result(
    results_dir: &Path,
    request_id: &str,
    content: &[u8],
) -> io::Result<PathBuf> {
    validate_request_id(request_id)?;
    let final_path = results_dir.join(format!("{}.out", request_id));
    let (temporary_path, mut temporary_file) =
        create_temporary_result(results_dir, request_id)?;

    let publish_result = (|| {
        temporary_file.write_all(content)?;
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
                        "result publish failed: {}; temporary cleanup failed: {}",
                        error, cleanup_error
                    ),
                )),
            }
        }
    }
}

fn reject_symlink(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        Err(invalid_record("request path must not be a symlink"))
    } else {
        Ok(())
    }
}

fn parse_unique_fields(content: &str) -> io::Result<HashMap<String, String>> {
    let mut fields = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid_record("request contains a malformed field"))?;
        let key = key.trim();
        let decoded_value = decode_kv_value(value.trim());
        if key.is_empty()
            || fields.insert(key.to_string(), decoded_value).is_some()
        {
            return Err(invalid_record(
                "request contains an empty or duplicate field",
            ));
        }
    }
    Ok(fields)
}

fn decode_kv_value(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('\\') => decoded.push('\\'),
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some(other) => {
                // Preserve pre-encoding records that used a literal
                // backslash before an unrelated character.
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }
    decoded
}

fn required_field<'a>(
    fields: &'a HashMap<String, String>,
    name: &str,
) -> io::Result<&'a str> {
    fields
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_record(&format!("request is missing {}", name)))
}

fn create_temporary_result(
    results_dir: &Path,
    request_id: &str,
) -> io::Result<(PathBuf, fs::File)> {
    for attempt in 0..TEMP_CREATE_ATTEMPTS {
        let path = results_dir.join(format!(
            ".{}.out.{}.{}.tmp",
            request_id,
            std::process::id(),
            attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o640)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "result temporary file namespace is exhausted",
    ))
}

fn invalid_record(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "boos-queue-record-{}-{}",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn request_body_id_must_match_its_queue_filename() {
        let directory = temporary_directory("identity");
        let request_path = directory.join("req-visible");
        std::fs::write(
            &request_path,
            "id=../../escaped\ncommand=help\nrequester=agent\nstatus=pending\n",
        )
        .unwrap();

        assert!(load_request(&request_path).is_err());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn duplicate_request_fields_are_rejected_instead_of_overriding_each_other() {
        let directory = temporary_directory("duplicate");
        let request_path = directory.join("req-duplicate");
        std::fs::write(
            &request_path,
            "id=req-duplicate\ncommand=help\ncommand=poweroff\nstatus=pending\n",
        )
        .unwrap();

        assert!(load_request(&request_path).is_err());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn request_path_cannot_be_a_symlink() {
        use std::os::unix::fs::symlink;

        let directory = temporary_directory("symlink");
        let target = directory.join("attacker-record");
        let request_path = directory.join("req-symlink");
        std::fs::write(
            &target,
            "id=req-symlink\ncommand=help\nstatus=pending\n",
        )
        .unwrap();
        symlink(&target, &request_path).unwrap();

        assert!(load_request(&request_path).is_err());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn result_publication_never_replaces_an_existing_result() {
        let directory = temporary_directory("result-collision");
        let existing = directory.join("req-fixed.out");
        std::fs::write(&existing, "original result").unwrap();

        let error = publish_result(&directory, "req-fixed", b"replacement").unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(existing).unwrap(),
            "original result"
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn published_result_is_readable_by_the_principal_group() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary_directory("result-mode");
        let path = publish_result(
            &directory,
            "req-readable",
            b"id=req-readable\nexit_code=0\n---\nok",
        )
        .unwrap();

        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn malformed_existing_result_is_not_accepted_as_completion_evidence() {
        let directory = temporary_directory("invalid-result");
        std::fs::write(
            directory.join("req-fixed.out"),
            "id=req-other\nexit_code=0\n---\nforged",
        )
        .unwrap();

        assert!(existing_result(&directory, "req-fixed").is_err());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn published_request_values_round_trip_through_the_queue_codec() {
        use crate::request_publish::{publish_request, RequestRecord};

        let directory = temporary_directory("round-trip");
        let record = RequestRecord {
            id: "req-round-trip",
            requester: "agent",
            command: "help",
            args: "path\\segment\nsecond line",
            submitted_at: 12.5,
            session_id: Some("session-a"),
        };
        let path = publish_request(&directory, &record).unwrap();

        let loaded = load_request(&path).unwrap();

        assert_eq!(loaded.args, record.args);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn spool_principal_overrides_untrusted_requester_field() {
        use crate::principal::PrincipalId;

        let directory = temporary_directory("principal-owner");
        let request_path = directory.join("req-owned");
        std::fs::write(
            &request_path,
            "id=req-owned\nrequester=forged\ncommand=help\nstatus=pending\n",
        )
        .unwrap();

        let request = load_request(&request_path).unwrap();
        let owned = request.with_principal(PrincipalId::parse("resident").unwrap());

        assert_eq!(owned.principal.as_str(), "resident");
        assert_eq!(owned.request.claimed_requester.as_deref(), Some("forged"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
