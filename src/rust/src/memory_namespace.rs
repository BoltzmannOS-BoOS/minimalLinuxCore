use std::io;
use std::path::{Path, PathBuf};

use crate::config;
use crate::principal::{self, PrincipalContext};

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
        let context = principal::current_context()?;
        Ok(Self::from_context(&context))
    }

    pub fn from_context(context: &PrincipalContext) -> Self {
        Self {
            root: context.memory_root(),
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
    use crate::principal::{
        resolve_context, PrincipalDefinition, PrincipalId,
    };
    use std::path::Path;

    fn principal_context(id: &str, uid: u32) -> crate::principal::PrincipalContext {
        let definition = PrincipalDefinition {
            id: PrincipalId::parse(id).unwrap(),
            user: format!("{id}-user"),
            uid,
            enabled: true,
        };
        resolve_context(&[definition], id, uid, Path::new("/runtime")).unwrap()
    }

    #[test]
    fn principal_contexts_never_share_memory_tiers() {
        let resident = MemoryNamespace::from_context(&principal_context("resident", 101));
        let debug = MemoryNamespace::from_context(&principal_context("debug", 100));

        assert_ne!(resident.working_path(), debug.working_path());
        assert_eq!(
            resident.archive_dir(),
            Path::new("/runtime/resident/memory/archive")
        );
        assert_eq!(
            debug.recent_dir(),
            Path::new("/runtime/debug/memory/recent")
        );
    }

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
