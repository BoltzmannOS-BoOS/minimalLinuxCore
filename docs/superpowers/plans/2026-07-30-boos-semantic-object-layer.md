# BoOS Semantic Object Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only semantic object ABI that projects the existing BoOS system and command capabilities into deterministic, model-readable objects, then provide a no-API A/B research harness.

**Architecture:** The L0 `world` module owns object values, validation, exact queries, and deterministic BoOS Object Format v1 encoding. L1 `world_sources` projects the existing command registry and capability policy without copying mutable state; L2 `world_command` owns the `schema/list/show` use case; the existing L3 dispatcher adds one match arm and one registered command.

**Tech Stack:** Rust 2021, standard library plus the repository's existing `ureq` dependency, BusyBox-compatible POSIX shell, `key=value` configuration and experiment records.

## Global Constraints

- Preserve every existing command and POSIX interface.
- Add no external dependency; `std + ureq` remains the complete Rust dependency set.
- Follow the existing multi-call Rust binary and command registry.
- Keep configuration and the new object wire format in UTF-8 `key=value` records.
- Expose no secret, environment variable, raw credential-bearing configuration, internal executable path, or unsafe path.
- Derive object state from the existing command registry and capability policy; do not create a second mutable database.
- Keep v1 read-only. Do not add intent execution, mutation, rollback, memory paging, embeddings, MCP, or A2A.
- Produce deterministic output: stable field order, sorted objects, sorted attributes, sorted relations, and sorted affordances.
- Preserve the existing exit-code contract: `0=allowed`, `1=denied`, `2=error`, `3=unknown`.
- Treat the user's existing untracked `.superpowers/` directory and BoOS screenshots as unrelated assets; do not stage them.

## File Map

| File | Responsibility |
|---|---|
| `src/rust/src/world.rs` | L0 object values, IDs, exact queries, escaping, schema and object rendering |
| `src/rust/src/world_sources.rs` | L1 safe projections from registry/config facts |
| `src/rust/src/world_command.rs` | L2 `world schema/list/show` parsing and exit behavior |
| `src/rust/src/registry.rs` | Preserve `load_commands`; add fallible registry loading so absence is observable |
| `src/rust/src/main.rs` | Declare the three new modules |
| `src/rust/src/exec.rs` | L3 help text and one builtin delegation arm |
| `rootfs/etc/boos/commands/world.cmd` | Public command registration |
| `rootfs/etc/boos/capabilities.conf` | Default read-only `allow_world=1` policy |
| `tests/suite/integration/integration-test.sh` | Gateway smoke assertions for the semantic ABI |
| `tests/research/semantic-object-view/*` | Prompts, tasks, run schema, validator, and research instructions |
| `README.md` | Link the new research direction from the project entry point |
| `docs/PROJECT-OVERVIEW.md` | Record the semantic ABI as the current experimental direction |
| `tests/suite/README.md` | Index the new research harness and refreshed local verification |

---

### Task 1: Deterministic World Object Model

**Files:**
- Create: `src/rust/src/world.rs`
- Modify: `src/rust/src/main.rs:4-19`

**Interfaces:**
- Consumes: Rust standard library only.
- Produces:
  - `pub const WORLD_SCHEMA: &str`
  - `pub const SUPPORTED_KINDS: &[&str]`
  - `pub struct NamedValue`
  - `pub struct WorldObject`
  - `pub enum WorldError`
  - `pub fn validate_object_id(id: &str) -> Result<(), WorldError>`
  - `pub fn render_schema() -> String`
  - `pub fn render_object(object: &WorldObject) -> String`
  - `pub fn render_objects(objects: &[WorldObject]) -> String`
  - `pub fn list_objects<'a>(objects: &'a [WorldObject], kind: Option<&str>) -> Result<Vec<&'a WorldObject>, WorldError>`
  - `pub fn find_object<'a>(objects: &'a [WorldObject], id: &str) -> Result<&'a WorldObject, WorldError>`

- [ ] **Step 1: Declare the module and write failing L0 tests**

Add `mod world;` after `mod registry;` in `src/rust/src/main.rs`.

Create `src/rust/src/world.rs` with the public types and tests first. Use this exact shape for the types so later tasks compile against one contract:

```rust
pub const WORLD_SCHEMA: &str = "boos.world.v1";
pub const SUPPORTED_KINDS: &[&str] = &["capability", "system"];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NamedValue {
    pub name: String,
    pub value: String,
}

impl NamedValue {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self { name: name.into(), value: value.into() }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorldObject {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub state: String,
    pub revision: Option<String>,
    pub provenance: String,
    pub summary: String,
    pub attributes: Vec<NamedValue>,
    pub relations: Vec<NamedValue>,
    pub affordances: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorldError {
    InvalidId(String),
    UnsupportedKind(String),
    NotFound(String),
}
```

Add tests covering the public behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn capability(id: &str) -> WorldObject {
        WorldObject {
            id: id.to_string(),
            kind: "capability".to_string(),
            label: id.split(':').nth(1).unwrap().to_string(),
            state: "enabled".to_string(),
            revision: None,
            provenance: "boos.command-registry".to_string(),
            summary: "test capability".to_string(),
            attributes: vec![],
            relations: vec![],
            affordances: vec!["invoke".to_string(), "inspect".to_string()],
        }
    }

    #[test]
    fn validates_stable_object_ids() {
        assert_eq!(validate_object_id("system:boos"), Ok(()));
        assert_eq!(validate_object_id("capability:read-file"), Ok(()));
        assert!(matches!(validate_object_id("read-file"), Err(WorldError::InvalidId(_))));
        assert!(matches!(validate_object_id("capability:"), Err(WorldError::InvalidId(_))));
        assert!(matches!(validate_object_id("Capability:help"), Err(WorldError::InvalidId(_))));
        assert!(matches!(validate_object_id("capability:../secret"), Err(WorldError::InvalidId(_))));
    }

    #[test]
    fn renders_reserved_characters_and_stable_field_order() {
        let mut object = capability("capability:read-file");
        object.summary = "line1\nline=2\\tail\r".to_string();
        object.attributes = vec![
            NamedValue::new("z", "last"),
            NamedValue::new("a", "first"),
        ];
        object.relations = vec![
            NamedValue::new("member_of", "system:boos"),
        ];

        let rendered = render_object(&object);
        assert_eq!(
            rendered,
            "schema=boos.world.v1\n\
             id=capability:read-file\n\
             kind=capability\n\
             label=read-file\n\
             state=enabled\n\
             provenance=boos.command-registry\n\
             summary=line1\\nline\\=2\\\\tail\\r\n\
             attribute.a=first\n\
             attribute.z=last\n\
             relation.member_of=system:boos\n\
             affordance.0=inspect\n\
             affordance.1=invoke"
        );
    }

    #[test]
    fn sorts_collections_and_queries_exactly() {
        let objects = vec![
            capability("capability:write-file"),
            capability("capability:help"),
            WorldObject {
                id: "system:boos".to_string(),
                kind: "system".to_string(),
                label: "BoOS".to_string(),
                state: "ready".to_string(),
                revision: Some("1".to_string()),
                provenance: "boos.runtime".to_string(),
                summary: "AI-native control layer".to_string(),
                attributes: vec![],
                relations: vec![],
                affordances: vec!["inspect".to_string()],
            },
        ];

        let listed = list_objects(&objects, Some("capability")).unwrap();
        assert_eq!(listed[0].id, "capability:help");
        assert_eq!(listed[1].id, "capability:write-file");
        assert_eq!(find_object(&objects, "system:boos").unwrap().label, "BoOS");
        assert!(matches!(
            list_objects(&objects, Some("service")),
            Err(WorldError::UnsupportedKind(kind)) if kind == "service"
        ));
        assert!(matches!(
            find_object(&objects, "capability:missing"),
            Err(WorldError::NotFound(id)) if id == "capability:missing"
        ));
        assert!(matches!(
            find_object(&objects, "service:web"),
            Err(WorldError::UnsupportedKind(kind)) if kind == "service"
        ));
    }

    #[test]
    fn renders_schema_as_self_description() {
        let schema = render_schema();
        assert!(schema.contains("schema=boos.world.v1"));
        assert!(schema.contains("supported_kind.0=capability"));
        assert!(schema.contains("supported_kind.1=system"));
        assert!(schema.contains("id_convention=<kind>:<local-name>"));
        assert!(schema.contains("escaping=\\\\ \\\\n \\\\r \\\\="));
    }
}
```

- [ ] **Step 2: Run the focused tests and verify the expected failure**

Run:

```bash
cd src/rust
cargo test world::tests -- --nocapture
```

Expected: compilation fails because `validate_object_id`, `render_object`,
`render_schema`, `list_objects`, and `find_object` are not defined.

- [ ] **Step 3: Implement validation, encoding, and exact queries**

Implement the missing functions in `world.rs` with these rules:

```rust
fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || byte == b'-'
                || byte == b'.'
        })
}

pub fn validate_object_id(id: &str) -> Result<(), WorldError> {
    let mut parts = id.split(':');
    let kind = parts.next().unwrap_or("");
    let local_name = parts.next().unwrap_or("");
    if parts.next().is_some() || !valid_component(kind) || !valid_component(local_name) {
        return Err(WorldError::InvalidId(id.to_string()));
    }
    Ok(())
}

fn escape_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '=' => escaped.push_str("\\="),
            _ => escaped.push(character),
        }
    }
    escaped
}
```

`render_object` must emit core fields in schema order, clone and sort
`attributes` and `relations` by `(name, value)`, clone/sort/deduplicate
`affordances`, and number affordances from zero. `render_objects` must sort
objects by ID and join records with exactly one blank line. `list_objects` must
reject kinds outside `SUPPORTED_KINDS` and return references sorted by ID.
`find_object` must validate the ID, reject an ID whose kind is outside
`SUPPORTED_KINDS`, and only then perform exact lookup. `render_object` must emit
`revision` immediately after `state` when it is present.

Use this exact schema response:

```rust
pub fn render_schema() -> String {
    [
        "schema=boos.world.v1",
        "record_format=key=value records separated by one blank line",
        "id_convention=<kind>:<local-name>",
        "supported_kind.0=capability",
        "supported_kind.1=system",
        "escaping=\\\\ \\\\n \\\\r \\\\=",
    ].join("\n")
}
```

- [ ] **Step 4: Run focused tests and formatting**

Run:

```bash
cd src/rust
cargo fmt -- --check
cargo test world::tests -- --nocapture
```

Expected: formatting check and all four world tests pass.

- [ ] **Step 5: Commit the L0 object model**

```bash
git add src/rust/src/main.rs src/rust/src/world.rs
git commit -m "feat: add semantic world object model"
```

---

### Task 2: Authoritative Registry Projection

**Files:**
- Modify: `src/rust/src/registry.rs:24-88`
- Create: `src/rust/src/world_sources.rs`
- Modify: `src/rust/src/main.rs:4-20`

**Interfaces:**
- Consumes:
  - `registry::Command`
  - `world::{NamedValue, WorldObject, validate_object_id}`
- Produces:
  - `pub fn try_load_commands() -> std::io::Result<Vec<Command>>`
  - `pub fn load_world() -> Vec<WorldObject>`
  - `pub fn project_world<F>(commands: Option<&[Command]>, is_enabled: F) -> Vec<WorldObject> where F: Fn(&str) -> bool`

- [ ] **Step 1: Write failing registry-availability and projection tests**

In `registry.rs`, add a test that loads a caller-provided directory:

```rust
#[test]
fn test_load_commands_from_missing_directory_reports_error() {
    let missing = std::env::temp_dir().join("boos-test-missing-command-dir");
    let _ = std::fs::remove_dir_all(&missing);
    assert!(load_commands_from_dir(&missing).is_err());
}
```

Declare `mod world_sources;` after `mod world;` in `main.rs`.

Create `world_sources.rs` with these tests:

```rust
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
    fn projects_enabled_and_disabled_capabilities_without_dispatch_details() {
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
    fn skips_invalid_semantic_ids_without_turning_them_into_paths() {
        let commands = vec![command("../secret", "allow_bad", "/tmp/secret")];
        let objects = project_world(Some(&commands), |_| true);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "system:boos");
    }
}
```

- [ ] **Step 2: Run focused tests and verify the expected failure**

Run:

```bash
cd src/rust
cargo test registry::tests::test_load_commands_from_missing_directory_reports_error -- --nocapture
cargo test world_sources::tests -- --nocapture
```

Expected: compilation fails because `load_commands_from_dir` and
`project_world` are not defined.

- [ ] **Step 3: Make registry absence observable without breaking callers**

Refactor the existing loader in `registry.rs`:

```rust
fn load_commands_from_dir(dir: &Path) -> std::io::Result<Vec<Command>> {
    let mut commands = Vec::new();
    let entries = fs::read_dir(dir)?;

    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.extension().map_or(false, |extension| extension == "cmd") {
            let kv = parse_kv_file(&path);
            let name = kv.get("name").cloned().unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            let enable_flag = kv.get("enable_flag")
                .or_else(|| kv.get("capability"))
                .cloned()
                .unwrap_or_default();
            let description = kv.get("description").cloned().unwrap_or_default();
            let exec = kv.get("exec").cloned().unwrap_or_default();
            let params = kv.get("params")
                .map(|parameters| parse_params(parameters))
                .unwrap_or_default();

            commands.push(Command {
                name,
                enable_flag,
                description,
                exec,
                params,
            });
        }
    }

    commands.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(commands)
}

pub fn try_load_commands() -> std::io::Result<Vec<Command>> {
    load_commands_from_dir(Path::new(config::CMD_DIR))
}

pub fn load_commands() -> Vec<Command> {
    try_load_commands().unwrap_or_default()
}
```

This preserves the behavior of every current `load_commands()` caller while
allowing the semantic layer to distinguish absence from an empty registry.

- [ ] **Step 4: Implement safe system and capability projections**

Implement `world_sources.rs` with this flow:

```rust
use crate::registry::{self, Command};
use crate::world::{self, NamedValue, WorldObject};

pub fn load_world() -> Vec<WorldObject> {
    match registry::try_load_commands() {
        Ok(commands) => project_world(Some(&commands), registry::is_enabled),
        Err(_) => project_world(None, |_| false),
    }
}

pub fn project_world<F>(
    commands: Option<&[Command]>,
    is_enabled: F,
) -> Vec<WorldObject>
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

    let projection_state = if commands.is_some() { "available" } else { "unavailable" };
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
        state: if commands.is_some() { "ready" } else { "degraded" }.to_string(),
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
                    if parameter.required { "required" } else { "optional" }
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
```

Do not read or serialize `Command.exec`. Do not serialize `enable_flag`.

- [ ] **Step 5: Run focused and regression tests**

Run:

```bash
cd src/rust
cargo fmt -- --check
cargo test registry::tests -- --nocapture
cargo test world_sources::tests -- --nocapture
cargo test world::tests -- --nocapture
```

Expected: all registry, source-projection, and L0 tests pass.

- [ ] **Step 6: Commit the source projection**

```bash
git add src/rust/src/main.rs src/rust/src/registry.rs src/rust/src/world_sources.rs
git commit -m "feat: project BoOS registry into semantic objects"
```

---

### Task 3: Read-Only World Command Flow

**Files:**
- Create: `src/rust/src/world_command.rs`
- Modify: `src/rust/src/main.rs:4-21`

**Interfaces:**
- Consumes:
  - `world::{find_object, list_objects, render_object, render_objects, render_schema, WorldError, WorldObject}`
  - `world_sources::load_world`
  - `config::{EXIT_ALLOWED, EXIT_ERROR, EXIT_UNKNOWN}`
- Produces:
  - `pub fn render(args: &str, objects: &[WorldObject]) -> Result<String, CommandFailure>`
  - `pub fn run(args: &str) -> i32`
  - `pub struct CommandFailure { pub exit_code: i32, pub message: String }`

- [ ] **Step 1: Declare the module and write failing command-contract tests**

Add `mod world_command;` after `mod world_sources;` in `main.rs`.

Create `world_command.rs` with:

```rust
use crate::config;
use crate::world::{self, WorldError, WorldObject};
use crate::world_sources;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CommandFailure {
    pub exit_code: i32,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(render("schema", &objects).unwrap().contains("schema=boos.world.v1"));

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
    fn rejects_missing_extra_unknown_and_malformed_arguments() {
        let objects = catalog();
        assert_eq!(render("", &objects).unwrap_err().exit_code, config::EXIT_ERROR);
        assert_eq!(render("schema extra", &objects).unwrap_err().exit_code, config::EXIT_ERROR);
        assert_eq!(render("list capability extra", &objects).unwrap_err().exit_code, config::EXIT_ERROR);
        assert_eq!(render("show", &objects).unwrap_err().exit_code, config::EXIT_ERROR);
        assert_eq!(render("unknown", &objects).unwrap_err().exit_code, config::EXIT_ERROR);
        assert_eq!(
            render("show capability:missing", &objects).unwrap_err().exit_code,
            config::EXIT_UNKNOWN
        );
        assert_eq!(
            render("show ../secret", &objects).unwrap_err().exit_code,
            config::EXIT_ERROR
        );
    }
}
```

- [ ] **Step 2: Run command tests and verify the expected failure**

Run:

```bash
cd src/rust
cargo test world_command::tests -- --nocapture
```

Expected: compilation fails because `render` is not defined.

- [ ] **Step 3: Implement exact command parsing and error mapping**

Implement:

```rust
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
            message: format!("Unknown world object: {id}"),
        },
        WorldError::InvalidId(id) => CommandFailure {
            exit_code: config::EXIT_ERROR,
            message: format!("Malformed world object ID: {id}"),
        },
        WorldError::UnsupportedKind(kind) => CommandFailure {
            exit_code: config::EXIT_ERROR,
            message: format!("Unsupported world object kind: {kind}"),
        },
    }
}

pub fn render(
    args: &str,
    objects: &[WorldObject],
) -> Result<String, CommandFailure> {
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
            println!("{output}");
            config::EXIT_ALLOWED
        }
        Err(failure) => {
            eprintln!("{}", failure.message);
            failure.exit_code
        }
    }
}
```

The vector clone in the list flow is acceptable for the bounded command
registry and keeps L0 rendering ownership simple. Do not add generic command
frameworks or lifetime-heavy abstractions to remove it.

- [ ] **Step 4: Run focused tests and the complete Rust suite**

Run:

```bash
cd src/rust
cargo fmt -- --check
cargo test world_command::tests -- --nocapture
cargo test
```

Expected: command tests and the complete existing suite pass.

- [ ] **Step 5: Commit the command flow**

```bash
git add src/rust/src/main.rs src/rust/src/world_command.rs
git commit -m "feat: add read-only world query flow"
```

---

### Task 4: Wire the Semantic ABI into BoOS

**Files:**
- Modify: `src/rust/src/exec.rs:7-43`
- Modify: `src/rust/src/exec.rs:351-360`
- Create: `rootfs/etc/boos/commands/world.cmd`
- Modify: `rootfs/etc/boos/capabilities.conf:1-8`
- Modify: `tests/suite/integration/integration-test.sh:61-70`

**Interfaces:**
- Consumes: `world_command::run(args: &str) -> i32`.
- Produces: public `world schema`, `world list [kind]`, and
  `world show <object-id>` commands behind `allow_world`.

- [ ] **Step 1: Add failing public-interface assertions**

Add these checks to the basic command section of
`tests/suite/integration/integration-test.sh`:

```sh
check "world schema"     "schema=boos.world.v1"    "world" "schema"
check "world root"       "id=system:boos"          "world" "show" "system:boos"
check "world capability" "id=capability:help"      "world" "show" "capability:help"
check "world filter"     "kind=capability"          "world" "list" "capability"
check "world registered" "world"                    "commands"
```

- [ ] **Step 2: Verify the current binary does not expose the command**

Run:

```bash
sh -n tests/suite/integration/integration-test.sh
rg -n '^name=world$' rootfs/etc/boos/commands
```

Expected: shell syntax passes; `rg` exits with status 1 because `world.cmd`
does not exist.

- [ ] **Step 3: Register and enable the read-only capability**

Create `rootfs/etc/boos/commands/world.cmd`:

```text
name=world
enable_flag=allow_world
description=inspect the semantic BoOS world (schema, list, show)
exec=__builtin_world
params=subcommand:required,selector:optional
```

Add this line after `allow_caps=1` in `capabilities.conf`:

```text
allow_world=1
```

- [ ] **Step 4: Add only L3 wiring to the oversized dispatcher**

Add one help line after `caps` in `show_help()`:

```rust
println!("  world <query>       inspect semantic objects (schema|list|show)");
```

Add one builtin match arm after `__builtin_caps`:

```rust
"__builtin_world" => crate::world_command::run(args),
```

Do not parse arguments, load files, format records, or inspect policy in
`exec.rs`.

- [ ] **Step 5: Verify registration, unit behavior, shell syntax, and build**

Run:

```bash
cd src/rust
cargo fmt -- --check
cargo test
cargo check
cargo build
cd ../..
sh -n tests/suite/integration/integration-test.sh
rg -n '^name=world$|^enable_flag=allow_world$|^exec=__builtin_world$' \
  rootfs/etc/boos/commands/world.cmd
rg -n '^allow_world=1$' rootfs/etc/boos/capabilities.conf
```

Expected: every command exits 0. The gateway integration checks are authored
but are not claimed as executed unless a matching BoOS QEMU/native gateway is
running.

- [ ] **Step 6: Commit public integration**

```bash
git add \
  src/rust/src/exec.rs \
  rootfs/etc/boos/commands/world.cmd \
  rootfs/etc/boos/capabilities.conf \
  tests/suite/integration/integration-test.sh
git commit -m "feat: expose semantic world queries"
```

---

### Task 5: No-API A/B Research Harness and Documentation

**Files:**
- Create: `tests/research/semantic-object-view/README.md`
- Create: `tests/research/semantic-object-view/tasks.kv`
- Create: `tests/research/semantic-object-view/baseline-prompt.txt`
- Create: `tests/research/semantic-object-view/object-prompt.txt`
- Create: `tests/research/semantic-object-view/result.example.kv`
- Create: `tests/research/semantic-object-view/validate-result.sh`
- Modify: `README.md:113-137`
- Modify: `docs/PROJECT-OVERVIEW.md:150-169`
- Modify: `tests/suite/README.md:3-35`

**Interfaces:**
- Consumes: existing `help/status/caps` baseline and new `world` treatment.
- Produces: a reproducible experiment task set and validated result-record
  format without making network or model calls.

- [ ] **Step 1: Write the experiment tasks and paired prompts**

Create `tasks.kv`:

```text
schema=boos.semantic-object-tasks.v1
task.001=enumerate enabled and disabled capabilities
task.002=identify every required parameter for reading a file
task.003=identify the capability that executes an allowed system program
task.004=decide whether a disabled capability can currently be invoked
task.005=produce a compact machine-readable capability map
```

Create `baseline-prompt.txt`:

```text
You are operating BoOS through its existing command interface.
Start with help, status, and caps. Complete every task in tasks.kv.
Do not assume a capability exists or is enabled without observing evidence.
Record each environment command, observation, conclusion, and verification.
```

Create `object-prompt.txt`:

```text
You are operating BoOS through its semantic object interface.
Start with world schema and world list. Complete every task in tasks.kv.
Use world show for exact objects. Do not assume a capability exists or is
enabled without observing evidence. Record each environment command,
observation, conclusion, and verification.
```

- [ ] **Step 2: Write the result format and validator**

Create `result.example.kv`:

```text
schema=boos.semantic-object-experiment.v1
run_id=example-object-run
variant=object
model=example-model
model_version=example-version
temperature=0
task_set=tasks.kv
prompt_path=object-prompt.txt
trace_path=replace-with-recorded-trace.txt
completed_tasks=0
total_tasks=5
environment_interactions=0
observation_bytes=0
incorrect_capability_assumptions=0
invalid_command_attempts=0
skipped_verifications=0
notes=Example schema record; do not treat as an experiment result.
```

Create `validate-result.sh`:

```sh
#!/bin/sh
set -eu

result_file="${1:?usage: validate-result.sh <result.kv>}"

required_keys="
schema
run_id
variant
model
model_version
temperature
task_set
prompt_path
trace_path
completed_tasks
total_tasks
environment_interactions
observation_bytes
incorrect_capability_assumptions
invalid_command_attempts
skipped_verifications
"

value_for() {
    key="$1"
    sed -n "s/^${key}=//p" "$result_file" | head -n 1
}

for key in $required_keys; do
    value="$(value_for "$key")"
    if [ -z "$value" ]; then
        echo "missing required field: $key" >&2
        exit 1
    fi
done

if [ "$(value_for schema)" != "boos.semantic-object-experiment.v1" ]; then
    echo "unsupported result schema" >&2
    exit 1
fi

case "$(value_for variant)" in
    baseline|object) ;;
    *)
        echo "variant must be baseline or object" >&2
        exit 1
        ;;
esac

numeric_keys="
completed_tasks
total_tasks
environment_interactions
observation_bytes
incorrect_capability_assumptions
invalid_command_attempts
skipped_verifications
"

for key in $numeric_keys; do
    value="$(value_for "$key")"
    case "$value" in
        *[!0-9]*)
            echo "$key must be a non-negative integer" >&2
            exit 1
            ;;
    esac
done

echo "valid semantic object experiment result"
```

Mark it executable:

```bash
chmod +x tests/research/semantic-object-view/validate-result.sh
```

- [ ] **Step 3: Verify the validator accepts the example and rejects malformed data**

Run:

```bash
tests/research/semantic-object-view/validate-result.sh \
  tests/research/semantic-object-view/result.example.kv

invalid_result="$(mktemp)"
sed '/^variant=/d' \
  tests/research/semantic-object-view/result.example.kv > "$invalid_result"
if tests/research/semantic-object-view/validate-result.sh "$invalid_result"; then
  echo "validator accepted a missing variant" >&2
  exit 1
fi
rm -f "$invalid_result"
```

Expected: the example prints `valid semantic object experiment result`; the
second validation fails with `missing required field: variant`; the overall
shell block exits 0.

- [ ] **Step 4: Document experimental procedure without claiming results**

Create `tests/research/semantic-object-view/README.md` with these required
sections:

```markdown
# Semantic Object View A/B Experiment

## Hypothesis

For the same model and task set, the BoOS semantic object view reduces
environment interactions, observation bytes, and incorrect capability
assumptions compared with the existing command-oriented discovery interface.

## Controlled Variables

Use the same BoOS image, model name and version, temperature, task set, tool
transport, maximum interactions, and completion criteria for both variants.
Run at least three repetitions per model and do not mix results from changed
BoOS images.

## Procedure

1. Record the BoOS git commit and image identifier.
2. Run `baseline-prompt.txt` with only the existing command interface.
3. Preserve the complete raw trace and fill one result record.
4. Reset to the same initial image state.
5. Run `object-prompt.txt` with the semantic object interface.
6. Preserve the complete raw trace and fill one result record.
7. Validate both records with `validate-result.sh`.
8. Compare individual metrics; do not report a conclusion from the example
   record.

## Metrics

Define completion against all five tasks in `tasks.kv`. Count every submitted
environment command as one interaction. Count only environment observations,
not prompt or model output, in `observation_bytes`. Mark assumptions and
skipped verification by reviewing the raw trace against authoritative object
or command state.

## Result Integrity

Never overwrite raw traces. Result records must name the model version,
temperature, prompt, task set, and trace. The harness makes no API calls and
contains no measured result by default.
```

Add a `## Current Research: Semantic Object Layer` section to `README.md`
after Architecture. Link:

- the design spec;
- this implementation plan;
- the A/B harness.

State that existing commands remain the control group and that no result is
claimed before recorded model runs.

Add `## 当前研究方向：Semantic Object Layer（2026-07-30）` before the
technical summary in `docs/PROJECT-OVERVIEW.md`. Describe the object ABI,
read-only v1 scope, and the hypothesis in at most two short paragraphs plus
three links.

Add a Research row to `tests/suite/README.md` and update the unit-test count
only from the actual final `cargo test` output.

- [ ] **Step 5: Run final local verification**

Run:

```bash
cd src/rust
cargo fmt -- --check
cargo test
cargo check
cargo build
cd ../..
sh -n tests/suite/integration/integration-test.sh
sh -n tests/research/semantic-object-view/validate-result.sh
tests/research/semantic-object-view/validate-result.sh \
  tests/research/semantic-object-view/result.example.kv
git diff --check
```

Expected: all commands exit 0. Record the exact Rust test count in
`tests/suite/README.md`. Do not claim that the gateway integration suite or
paid-model A/B runs were executed unless they were actually run.

- [ ] **Step 6: Review dependency direction and scope**

Run:

```bash
rg -n 'crate::(exec|world_command|world_sources|registry)' \
  src/rust/src/world.rs \
  src/rust/src/world_sources.rs \
  src/rust/src/world_command.rs
rg -n 'ureq|serde|tokio|clap' \
  src/rust/src/world.rs \
  src/rust/src/world_sources.rs \
  src/rust/src/world_command.rs
git status --short
```

Expected:

- `world.rs` has no higher-layer dependency;
- `world_sources.rs` depends only on `world` and `registry`;
- `world_command.rs` depends on `world` and `world_sources`;
- no new dependency reference appears;
- status lists only intended task files plus the user's pre-existing untracked
  assets.

- [ ] **Step 7: Commit the research harness and documentation**

```bash
git add \
  README.md \
  docs/PROJECT-OVERVIEW.md \
  tests/suite/README.md \
  tests/research/semantic-object-view
git commit -m "test: add semantic object view experiment"
```

---

## Completion Review

After Task 5:

1. Compare the implementation against every section of
   `docs/superpowers/specs/2026-07-30-boos-semantic-object-layer-design.md`.
2. Confirm `Command.exec`, capability flag names, secrets, environment
   variables, and arbitrary paths never appear in world output.
3. Confirm disabled objects remain visible and omit the `invoke` affordance.
4. Confirm missing registry state is `degraded/unavailable`, not an empty
   healthy world.
5. Confirm no behavior was added to `exec.rs` beyond help and delegation.
6. Confirm no transaction, memory, embedding, agent-loop, MCP, or A2A scope was
   introduced.
7. Confirm every claimed check appears in the final command output.
