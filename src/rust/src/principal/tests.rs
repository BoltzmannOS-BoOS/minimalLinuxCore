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
        gid: uid,
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

    let error =
        resolve_context(&definitions, "resident", 100, Path::new("/runtime")).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn disabled_principal_cannot_be_resolved() {
    let mut resident = definition("resident", 101);
    resident.enabled = false;

    let error =
        resolve_context(&[resident], "resident", 101, Path::new("/runtime")).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn derives_paths_below_the_validated_principal_root() {
    let definitions = vec![definition("resident", 101)];

    let context =
        resolve_context(&definitions, "resident", 101, Path::new("/runtime")).unwrap();

    assert_eq!(context.id().as_str(), "resident");
    assert_eq!(context.uid(), 101);
    assert_eq!(context.gid(), 101);
    assert_eq!(context.user(), "resident-user");
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
            "id=resident\nuser=resident-a\nuid=101\ngid=101\nenabled=1\n",
        ),
        (
            "b.principal",
            "id=resident\nuser=resident-b\nuid=102\ngid=102\nenabled=1\n",
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
            "id=resident\nuser=boos-agent\ngid=101\nenabled=1\n",
        ),
        (
            "invalid-enabled.principal",
            "id=debug\nuser=boos-gateway\nuid=100\ngid=100\nenabled=yes\n",
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
fn definition_requires_a_group_for_privilege_drop() {
    let directory = FixtureDirectory::new(&[(
        "missing-gid.principal",
        "id=resident\nuser=boos-agent\nuid=101\nenabled=1\n",
    )]);

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
            "id=resident\nuser=boos-agent\nuid=101\ngid=101\nenabled=1\n",
        ),
        (
            "a.principal",
            "id=debug\nuser=boos-gateway\nuid=100\ngid=100\nenabled=1\n",
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
