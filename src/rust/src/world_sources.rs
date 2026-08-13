use crate::registry::{self, Command};
use crate::world::{self, NamedValue, WorldObject};

pub fn load_world() -> Vec<WorldObject> {
    match registry::try_load_commands() {
        Ok(commands) => project_world(Some(&commands), registry::is_enabled),
        Err(_) => project_world(None, |_| false),
    }
}

pub fn project_world<F>(commands: Option<&[Command]>, is_enabled: F) -> Vec<WorldObject>
where
    F: Fn(&str) -> bool,
{
    let visible_commands: Vec<&Command> = commands
        .unwrap_or(&[])
        .iter()
        .filter(|command| {
            world::validate_object_id(&format!("capability:{}", command.name)).is_ok()
        })
        .collect();

    let projection_state = if commands.is_some() {
        "available"
    } else {
        "unavailable"
    };
    let mut system_relations = Vec::new();
    for (index, command) in visible_commands.iter().enumerate() {
        system_relations.push(NamedValue::new(
            format!("capability.{index:03}"),
            format!("capability:{}", command.name),
        ));
    }

    let mut objects = vec![WorldObject {
        id: "system:boos".to_string(),
        kind: "system".to_string(),
        label: "BoOS".to_string(),
        state: if commands.is_some() {
            "ready"
        } else {
            "degraded"
        }
        .to_string(),
        revision: None,
        provenance: "boos.runtime".to_string(),
        summary: "AI-native semantic control layer on Linux".to_string(),
        attributes: vec![
            NamedValue::new("projection.capabilities", projection_state),
            NamedValue::new("semantic_abi", world::WORLD_SCHEMA),
        ],
        relations: system_relations,
        affordances: vec!["inspect".to_string()],
    }];

    for command in visible_commands {
        let enabled = is_enabled(&command.enable_flag);
        let mut attributes = Vec::new();
        for (index, parameter) in command.params.iter().enumerate() {
            attributes.push(NamedValue::new(
                format!("parameter.{index:03}"),
                format!(
                    "{}:{}",
                    parameter.name,
                    if parameter.required {
                        "required"
                    } else {
                        "optional"
                    }
                ),
            ));
        }

        let mut affordances = vec!["inspect".to_string()];
        if enabled {
            affordances.push("invoke".to_string());
        }

        objects.push(WorldObject {
            id: format!("capability:{}", command.name),
            kind: "capability".to_string(),
            label: command.name.clone(),
            state: if enabled { "enabled" } else { "disabled" }.to_string(),
            revision: None,
            provenance: "boos.command-registry".to_string(),
            summary: command.description.clone(),
            attributes,
            relations: vec![NamedValue::new("member_of", "system:boos")],
            affordances,
        });
    }

    objects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Command, ParamDef};
    use crate::world::render_objects;

    fn command(name: &str, flag: &str, exec: &str) -> Command {
        Command {
            name: name.to_string(),
            enable_flag: flag.to_string(),
            description: format!("{} description", name),
            exec: exec.to_string(),
            params: vec![ParamDef {
                name: "path".to_string(),
                required: true,
            }],
        }
    }

    #[test]
    fn projects_capability_state_without_dispatch_details() {
        let commands = vec![
            command("read-file", "allow_read_file", "__builtin_read_file"),
            command("reset", "allow_reset", "__builtin_reset"),
        ];
        let objects = project_world(Some(&commands), |flag| flag == "allow_read_file");
        let rendered = render_objects(&objects);

        assert!(rendered.contains("id=system:boos"));
        assert!(rendered.contains("attribute.projection.capabilities=available"));
        assert!(rendered.contains("id=capability:read-file"));
        assert!(rendered.contains("state=enabled"));
        assert!(rendered.contains("attribute.parameter.000=path:required"));
        assert!(rendered.contains("affordance.1=invoke"));
        assert!(rendered.contains("id=capability:reset"));
        assert!(rendered.contains("state=disabled"));
        assert!(!rendered.contains("__builtin_"));
        assert!(!rendered.contains("allow_read_file"));
    }

    #[test]
    fn reports_unavailable_registry_on_the_root_object() {
        let objects = project_world(None, |_| false);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "system:boos");
        assert_eq!(objects[0].state, "degraded");
        assert!(objects[0].attributes.iter().any(|field| {
            field.name == "projection.capabilities" && field.value == "unavailable"
        }));
    }

    #[test]
    fn rejects_command_names_that_are_not_semantic_ids() {
        let commands = vec![command("../secret", "allow_bad", "/tmp/secret")];
        let objects = project_world(Some(&commands), |_| true);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "system:boos");
    }

    #[test]
    fn loading_the_host_world_always_returns_the_system_root() {
        let objects = load_world();
        assert!(objects.iter().any(|object| object.id == "system:boos"));
    }
}
