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
}
