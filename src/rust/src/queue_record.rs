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
}
