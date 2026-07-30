use std::io;
use std::path::{Path, PathBuf};

use crate::config;

mod definition;
mod process;

pub use definition::{load_definitions, load_definitions_from};
pub use process::current_context;

#[cfg(test)]
use process::parse_effective_uid;

#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PrincipalId(String);

impl PrincipalId {
    pub fn parse(value: &str) -> io::Result<Self> {
        if config::is_valid_runtime_id(value) {
            Ok(Self(value.to_string()))
        } else {
            Err(invalid_data("principal ID is invalid"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PrincipalDefinition {
    pub id: PrincipalId,
    pub user: String,
    pub uid: u32,
    pub gid: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PrincipalContext {
    definition: PrincipalDefinition,
    runtime_root: PathBuf,
}

impl PrincipalContext {
    pub fn id(&self) -> &PrincipalId {
        &self.definition.id
    }

    pub fn uid(&self) -> u32 {
        self.definition.uid
    }

    pub fn gid(&self) -> u32 {
        self.definition.gid
    }

    pub fn user(&self) -> &str {
        &self.definition.user
    }

    pub fn runtime_root(&self) -> &Path {
        &self.runtime_root
    }

    pub fn memory_root(&self) -> PathBuf {
        self.runtime_root.join("memory")
    }

    pub fn requests_dir(&self) -> PathBuf {
        self.runtime_root.join("requests")
    }

    pub fn results_dir(&self) -> PathBuf {
        self.runtime_root.join("results")
    }

    pub fn status_path(&self) -> PathBuf {
        self.runtime_root.join("status.kv")
    }
}

pub fn resolve_context(
    definitions: &[PrincipalDefinition],
    claimed_id: &str,
    effective_uid: u32,
    runtime_directory: &Path,
) -> io::Result<PrincipalContext> {
    let id = PrincipalId::parse(claimed_id)?;
    let definition = definitions
        .iter()
        .find(|definition| definition.id == id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "principal is not configured"))?;

    if !definition.enabled {
        return Err(permission_denied("principal is disabled"));
    }
    if definition.uid != effective_uid {
        return Err(permission_denied(
            "principal claim does not match the effective UID",
        ));
    }

    Ok(PrincipalContext {
        definition: definition.clone(),
        runtime_root: runtime_directory.join(id.as_str()),
    })
}

pub(crate) fn configured_context(
    definition: &PrincipalDefinition,
    runtime_directory: &Path,
) -> PrincipalContext {
    PrincipalContext {
        definition: definition.clone(),
        runtime_root: runtime_directory.join(definition.id.as_str()),
    }
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn permission_denied(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
mod tests;
