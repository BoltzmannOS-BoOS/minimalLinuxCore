use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config;

const PRINCIPAL_ENV: &str = "BOOS_PRINCIPAL_ID";
const LEGACY_AGENT_ENV: &str = "BOOS_AGENT_ID";

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

pub fn load_definitions() -> io::Result<Vec<PrincipalDefinition>> {
    load_definitions_from(Path::new(config::PRINCIPAL_CONFIG_DIR))
}

pub fn load_definitions_from(directory: &Path) -> io::Result<Vec<PrincipalDefinition>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("principal") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut definitions = Vec::with_capacity(paths.len());
    let mut seen_ids = HashSet::new();
    for path in paths {
        let fields = parse_definition_fields(&fs::read_to_string(&path)?)?;
        let definition = definition_from_fields(&fields)?;
        if !seen_ids.insert(definition.id.clone()) {
            return Err(invalid_data("duplicate principal ID"));
        }
        definitions.push(definition);
    }
    definitions.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(definitions)
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

pub fn current_context() -> io::Result<PrincipalContext> {
    let claimed_id = read_claimed_principal()?;
    let definitions = load_definitions()?;
    let status = fs::read_to_string("/proc/self/status")?;
    let effective_uid = parse_effective_uid(&status)?;
    resolve_context(
        &definitions,
        &claimed_id,
        effective_uid,
        Path::new(config::PRINCIPAL_RUNTIME_DIR),
    )
}

fn read_claimed_principal() -> io::Result<String> {
    match env::var(PRINCIPAL_ENV) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => match env::var(LEGACY_AGENT_ENV) {
            Ok(value) => Ok(value),
            Err(env::VarError::NotPresent) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "no BoOS principal identity is present",
            )),
            Err(env::VarError::NotUnicode(_)) => {
                Err(invalid_data("legacy agent identity is not valid Unicode"))
            }
        },
        Err(env::VarError::NotUnicode(_)) => {
            Err(invalid_data("principal identity is not valid Unicode"))
        }
    }
}

fn parse_definition_fields(content: &str) -> io::Result<HashMap<String, String>> {
    let mut fields = HashMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid_data("principal definition line has no '='"))?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(invalid_data("principal definition has an invalid duplicate key"));
        }
    }
    Ok(fields)
}

fn definition_from_fields(
    fields: &HashMap<String, String>,
) -> io::Result<PrincipalDefinition> {
    for field in fields.keys() {
        if !matches!(field.as_str(), "id" | "user" | "uid" | "enabled") {
            return Err(invalid_data("principal definition has an unknown field"));
        }
    }

    let id = PrincipalId::parse(required_field(fields, "id")?)?;
    let user = required_field(fields, "user")?;
    if user.is_empty()
        || user
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == b'/')
    {
        return Err(invalid_data("principal user is invalid"));
    }
    let uid = required_field(fields, "uid")?
        .parse::<u32>()
        .map_err(|_| invalid_data("principal UID is invalid"))?;
    let enabled = match required_field(fields, "enabled")? {
        "0" => false,
        "1" => true,
        _ => return Err(invalid_data("principal enabled flag must be 0 or 1")),
    };

    Ok(PrincipalDefinition {
        id,
        user: user.to_string(),
        uid,
        enabled,
    })
}

fn required_field<'a>(
    fields: &'a HashMap<String, String>,
    name: &str,
) -> io::Result<&'a str> {
    fields
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_data("principal definition is missing a required field"))
}

pub(crate) fn parse_effective_uid(status: &str) -> io::Result<u32> {
    let value = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .and_then(|line| line.split_whitespace().nth(2))
        .ok_or_else(|| invalid_data("process status has no effective UID"))?;
    value
        .parse::<u32>()
        .map_err(|_| invalid_data("process effective UID is invalid"))
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn permission_denied(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FixtureDirectory {
        path: PathBuf,
    }

    impl FixtureDirectory {
        fn new(files: &[(&str, &str)]) -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "boos-principal-test-{}-{}",
                std::process::id(),
                suffix
            ));
            fs::create_dir_all(&path).unwrap();
            for (name, content) in files {
                fs::write(path.join(name), content).unwrap();
            }
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn definition(id: &str, uid: u32) -> PrincipalDefinition {
        PrincipalDefinition {
            id: PrincipalId::parse(id).unwrap(),
            user: format!("{}-user", id),
            uid,
            enabled: true,
        }
    }

    #[test]
    fn malformed_principal_ids_are_rejected() {
        for invalid in ["", ".", "..", "../escape", "has space", "a/b"] {
            assert!(
                PrincipalId::parse(invalid).is_err(),
                "{invalid:?} must not become a runtime path component"
            );
        }
    }

    #[test]
    fn rejects_claim_when_effective_uid_does_not_match_definition() {
        let definitions = vec![definition("resident", 101)];

        let error = resolve_context(
            &definitions,
            "resident",
            100,
            Path::new("/runtime"),
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn disabled_principal_cannot_be_resolved() {
        let mut resident = definition("resident", 101);
        resident.enabled = false;

        let error = resolve_context(
            &[resident],
            "resident",
            101,
            Path::new("/runtime"),
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn derives_paths_below_the_validated_principal_root() {
        let definitions = vec![definition("resident", 101)];

        let context = resolve_context(
            &definitions,
            "resident",
            101,
            Path::new("/runtime"),
        )
        .unwrap();

        assert_eq!(context.id().as_str(), "resident");
        assert_eq!(context.runtime_root(), Path::new("/runtime/resident"));
        assert_eq!(context.memory_root(), Path::new("/runtime/resident/memory"));
        assert_eq!(
            context.requests_dir(),
            Path::new("/runtime/resident/requests")
        );
        assert_eq!(
            context.results_dir(),
            Path::new("/runtime/resident/results")
        );
        assert_eq!(
            context.status_path(),
            Path::new("/runtime/resident/status.kv")
        );
    }

    #[test]
    fn duplicate_principal_ids_are_rejected() {
        let directory = FixtureDirectory::new(&[
            (
                "a.principal",
                "id=resident\nuser=resident-a\nuid=101\nenabled=1\n",
            ),
            (
                "b.principal",
                "id=resident\nuser=resident-b\nuid=102\nenabled=1\n",
            ),
        ]);

        assert_eq!(
            load_definitions_from(directory.path())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn definition_requires_complete_typed_fields() {
        let directory = FixtureDirectory::new(&[
            (
                "missing-uid.principal",
                "id=resident\nuser=boos-agent\nenabled=1\n",
            ),
            (
                "invalid-enabled.principal",
                "id=debug\nuser=boos-gateway\nuid=100\nenabled=yes\n",
            ),
        ]);

        assert_eq!(
            load_definitions_from(directory.path())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn definitions_are_loaded_in_stable_id_order() {
        let directory = FixtureDirectory::new(&[
            (
                "z.principal",
                "id=resident\nuser=boos-agent\nuid=101\nenabled=1\n",
            ),
            (
                "a.principal",
                "id=debug\nuser=boos-gateway\nuid=100\nenabled=1\n",
            ),
        ]);

        let definitions = load_definitions_from(directory.path()).unwrap();
        let ids: Vec<&str> = definitions
            .iter()
            .map(|definition| definition.id.as_str())
            .collect();

        assert_eq!(ids, vec!["debug", "resident"]);
    }

    #[test]
    fn effective_uid_parser_uses_the_effective_column() {
        let status = "Name:\tboos-agent\nUid:\t101\t202\t303\t404\n";

        assert_eq!(parse_effective_uid(status).unwrap(), 202);
    }
}
