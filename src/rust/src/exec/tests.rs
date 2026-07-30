use super::*;
use crate::principal::{
    resolve_context, PrincipalContext, PrincipalDefinition, PrincipalId,
};

fn principal_context(id: &str, uid: u32, runtime_root: &Path) -> PrincipalContext {
    let definition = PrincipalDefinition {
        id: PrincipalId::parse(id).unwrap(),
        user: format!("{id}-user"),
        uid,
        gid: uid,
        enabled: true,
    };
    resolve_context(&[definition], id, uid, runtime_root).unwrap()
}

#[test]
fn result_paths_only_expose_the_calling_principal_namespace() {
    let runtime_root = std::env::temp_dir().join(format!(
        "boos-exec-principal-results-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&runtime_root);
    let resident = principal_context("resident", 101, &runtime_root);
    let debug = principal_context("debug", 100, &runtime_root);
    fs::create_dir_all(resident.results_dir()).unwrap();
    fs::create_dir_all(debug.results_dir()).unwrap();
    fs::write(resident.results_dir().join("req-resident.out"), "resident").unwrap();
    fs::write(debug.results_dir().join("req-debug.out"), "debug").unwrap();

    let paths = result_paths(&resident).unwrap();

    assert_eq!(paths, vec![resident.results_dir().join("req-resident.out")]);
    fs::remove_dir_all(runtime_root).unwrap();
}

