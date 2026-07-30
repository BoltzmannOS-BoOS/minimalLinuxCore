use std::env;
use std::io;
use std::path::{Path, PathBuf};

use crate::config;

#[derive(Debug, Clone)]
pub struct MemoryNamespace {
    root: PathBuf,
}

impl MemoryNamespace {
    pub fn new(memory_root: &Path, agent_id: Option<&str>) -> io::Result<Self> {
        let root = match agent_id {
            None | Some("default") => memory_root.to_path_buf(),
            Some(id) => {
                validate_namespace_id(id)?;
                memory_root.join(id)
            }
        };
        Ok(Self { root })
    }

    pub fn from_environment() -> io::Result<Self> {
        match env::var("BOOS_AGENT_ID") {
            Ok(agent_id) => Self::new(Path::new(config::MEMORY_DIR), Some(&agent_id)),
            Err(env::VarError::NotPresent) => {
                Self::new(Path::new(config::MEMORY_DIR), None)
            }
            Err(env::VarError::NotUnicode(_)) => Err(invalid_namespace_id()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn working_path(&self) -> PathBuf {
        self.root.join("working.kv")
    }

    pub fn working_temp_path(&self, session_id: &str) -> io::Result<PathBuf> {
        validate_namespace_id(session_id)?;
        Ok(self.root.join(format!("working.{}.tmp", session_id)))
    }

    pub fn recent_dir(&self) -> PathBuf {
        self.root.join("recent")
    }

    pub fn archive_dir(&self) -> PathBuf {
        self.root.join("archive")
    }
}

fn validate_namespace_id(id: &str) -> io::Result<()> {
    if config::is_valid_runtime_id(id) {
        Ok(())
    } else {
        Err(invalid_namespace_id())
    }
}

fn invalid_namespace_id() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "agent and session IDs must be 1-64 ASCII letters, digits, '.', '-' or '_'",
    )
}

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
