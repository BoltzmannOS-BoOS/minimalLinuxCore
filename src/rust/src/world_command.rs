use crate::config;
use crate::world::{self, WorldError, WorldObject};
use crate::world_sources;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CommandFailure {
    pub exit_code: i32,
    pub message: String,
}

fn usage() -> CommandFailure {
    CommandFailure {
        exit_code: config::EXIT_ERROR,
        message: "Usage: world {schema|list [kind]|show <object-id>}".to_string(),
    }
}

fn map_world_error(error: WorldError) -> CommandFailure {
    match error {
        WorldError::NotFound(id) => CommandFailure {
            exit_code: config::EXIT_UNKNOWN,
            message: format!("Unknown world object: {}", id),
        },
        WorldError::InvalidId(id) => CommandFailure {
            exit_code: config::EXIT_ERROR,
            message: format!("Malformed world object ID: {}", id),
        },
        WorldError::UnsupportedKind(kind) => CommandFailure {
            exit_code: config::EXIT_ERROR,
            message: format!("Unsupported world object kind: {}", kind),
        },
    }
}

pub fn render(args: &str, objects: &[WorldObject]) -> Result<String, CommandFailure> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts.as_slice() {
        ["schema"] => Ok(world::render_schema()),
        ["list"] => world::list_objects(objects, None)
            .map(|selected| {
                let owned: Vec<WorldObject> = selected.into_iter().cloned().collect();
                world::render_objects(&owned)
            })
            .map_err(map_world_error),
        ["list", kind] => world::list_objects(objects, Some(kind))
            .map(|selected| {
                let owned: Vec<WorldObject> = selected.into_iter().cloned().collect();
                world::render_objects(&owned)
            })
            .map_err(map_world_error),
        ["show", id] => world::find_object(objects, id)
            .map(world::render_object)
            .map_err(map_world_error),
        _ => Err(usage()),
    }
}

pub fn run(args: &str) -> i32 {
    let objects = world_sources::load_world();
    match render(args, &objects) {
        Ok(output) => {
            println!("{}", output);
            config::EXIT_ALLOWED
        }
        Err(failure) => {
            eprintln!("{}", failure.message);
            failure.exit_code
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldObject;

    fn catalog() -> Vec<WorldObject> {
        vec![
            WorldObject {
                id: "system:boos".to_string(),
                kind: "system".to_string(),
                label: "BoOS".to_string(),
                state: "ready".to_string(),
                revision: None,
                provenance: "boos.runtime".to_string(),
                summary: "AI-native control layer".to_string(),
                attributes: vec![],
                relations: vec![],
                affordances: vec!["inspect".to_string()],
            },
            WorldObject {
                id: "capability:help".to_string(),
                kind: "capability".to_string(),
                label: "help".to_string(),
                state: "enabled".to_string(),
                revision: None,
                provenance: "boos.command-registry".to_string(),
                summary: "show help".to_string(),
                attributes: vec![],
                relations: vec![],
                affordances: vec!["inspect".to_string(), "invoke".to_string()],
            },
        ]
    }

    #[test]
    fn renders_schema_list_filter_and_exact_show() {
        let objects = catalog();
        assert!(render("schema", &objects)
            .unwrap()
            .contains("schema=boos.world.v1"));

        let all = render("list", &objects).unwrap();
        assert!(all.contains("id=system:boos"));
        assert!(all.contains("id=capability:help"));

        let capabilities = render("list capability", &objects).unwrap();
        assert!(!capabilities.contains("id=system:boos"));
        assert!(capabilities.contains("id=capability:help"));

        let shown = render("show capability:help", &objects).unwrap();
        assert!(shown.contains("id=capability:help"));
        assert!(!shown.contains("id=system:boos"));
    }

    #[test]
    fn rejects_invalid_command_shapes_and_queries() {
        let objects = catalog();
        assert_eq!(
            render("", &objects).unwrap_err().exit_code,
            config::EXIT_ERROR
        );
        assert_eq!(
            render("schema extra", &objects).unwrap_err().exit_code,
            config::EXIT_ERROR
        );
        assert_eq!(
            render("list capability extra", &objects)
                .unwrap_err()
                .exit_code,
            config::EXIT_ERROR
        );
        assert_eq!(
            render("show", &objects).unwrap_err().exit_code,
            config::EXIT_ERROR
        );
        assert_eq!(
            render("unknown", &objects).unwrap_err().exit_code,
            config::EXIT_ERROR
        );
        assert_eq!(
            render("show capability:missing", &objects)
                .unwrap_err()
                .exit_code,
            config::EXIT_UNKNOWN
        );
        assert_eq!(
            render("show ../secret", &objects).unwrap_err().exit_code,
            config::EXIT_ERROR
        );
        assert_eq!(
            render("show service:web", &objects).unwrap_err().exit_code,
            config::EXIT_ERROR
        );
    }

    #[test]
    fn runner_preserves_allowed_unknown_and_error_exit_codes() {
        assert_eq!(run("schema"), config::EXIT_ALLOWED);
        assert_eq!(run("show capability:missing"), config::EXIT_UNKNOWN);
        assert_eq!(run("unknown"), config::EXIT_ERROR);
    }
}
