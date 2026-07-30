#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn all_memory_tiers_share_the_validated_agent_root() {
        let namespace =
            MemoryNamespace::new(Path::new("/memory"), Some("agent-a")).unwrap();

        assert_eq!(namespace.working_path(), Path::new("/memory/agent-a/working.kv"));
        assert_eq!(namespace.recent_dir(), Path::new("/memory/agent-a/recent"));
        assert_eq!(namespace.archive_dir(), Path::new("/memory/agent-a/archive"));
    }

    #[test]
    fn path_traversal_agent_id_is_rejected() {
        let result = MemoryNamespace::new(Path::new("/memory"), Some("../escape"));

        assert!(result.is_err(), "agent identity must be one path component");
    }
}
