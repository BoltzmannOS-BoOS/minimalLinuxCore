pub const WORLD_SCHEMA: &str = "boos.world.v1";
pub const SUPPORTED_KINDS: &[&str] = &["capability", "system"];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NamedValue {
    pub name: String,
    pub value: String,
}

impl NamedValue {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
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

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.'
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

fn push_field(lines: &mut Vec<String>, key: &str, value: &str) {
    lines.push(format!("{}={}", key, escape_value(value)));
}

pub fn render_schema() -> String {
    [
        "schema=boos.world.v1",
        "record_format=key=value records separated by one blank line",
        "id_convention=<kind>:<local-name>",
        "supported_kind.0=capability",
        "supported_kind.1=system",
        "escaping=\\\\ \\\\n \\\\r \\\\=",
    ]
    .join("\n")
}

pub fn render_object(object: &WorldObject) -> String {
    let mut lines = Vec::new();
    push_field(&mut lines, "schema", WORLD_SCHEMA);
    push_field(&mut lines, "id", &object.id);
    push_field(&mut lines, "kind", &object.kind);
    push_field(&mut lines, "label", &object.label);
    push_field(&mut lines, "state", &object.state);
    if let Some(revision) = &object.revision {
        push_field(&mut lines, "revision", revision);
    }
    push_field(&mut lines, "provenance", &object.provenance);
    push_field(&mut lines, "summary", &object.summary);

    let mut attributes = object.attributes.clone();
    attributes.sort_by(|left, right| (&left.name, &left.value).cmp(&(&right.name, &right.value)));
    for attribute in attributes {
        push_field(
            &mut lines,
            &format!("attribute.{}", attribute.name),
            &attribute.value,
        );
    }

    let mut relations = object.relations.clone();
    relations.sort_by(|left, right| (&left.name, &left.value).cmp(&(&right.name, &right.value)));
    for relation in relations {
        push_field(
            &mut lines,
            &format!("relation.{}", relation.name),
            &relation.value,
        );
    }

    let mut affordances = object.affordances.clone();
    affordances.sort();
    affordances.dedup();
    for (index, affordance) in affordances.iter().enumerate() {
        push_field(&mut lines, &format!("affordance.{}", index), affordance);
    }

    lines.join("\n")
}

pub fn render_objects(objects: &[WorldObject]) -> String {
    let mut sorted: Vec<&WorldObject> = objects.iter().collect();
    sorted.sort_by(|left, right| left.id.cmp(&right.id));
    sorted
        .into_iter()
        .map(render_object)
        .collect::<Vec<String>>()
        .join("\n\n")
}

fn ensure_supported_kind(kind: &str) -> Result<(), WorldError> {
    if SUPPORTED_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(WorldError::UnsupportedKind(kind.to_string()))
    }
}

pub fn list_objects<'a>(
    objects: &'a [WorldObject],
    kind: Option<&str>,
) -> Result<Vec<&'a WorldObject>, WorldError> {
    if let Some(kind) = kind {
        ensure_supported_kind(kind)?;
    }

    let mut selected: Vec<&WorldObject> = objects
        .iter()
        .filter(|object| kind.map_or(true, |wanted| object.kind == wanted))
        .collect();
    selected.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(selected)
}

pub fn find_object<'a>(
    objects: &'a [WorldObject],
    id: &str,
) -> Result<&'a WorldObject, WorldError> {
    validate_object_id(id)?;
    let kind = id.split_once(':').map(|parts| parts.0).unwrap_or("");
    ensure_supported_kind(kind)?;
    objects
        .iter()
        .find(|object| object.id == id)
        .ok_or_else(|| WorldError::NotFound(id.to_string()))
}

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
        assert!(matches!(
            validate_object_id("read-file"),
            Err(WorldError::InvalidId(_))
        ));
        assert!(matches!(
            validate_object_id("capability:"),
            Err(WorldError::InvalidId(_))
        ));
        assert!(matches!(
            validate_object_id("Capability:help"),
            Err(WorldError::InvalidId(_))
        ));
        assert!(matches!(
            validate_object_id("capability:../secret"),
            Err(WorldError::InvalidId(_))
        ));
    }

    #[test]
    fn renders_reserved_characters_and_stable_field_order() {
        let mut object = capability("capability:read-file");
        object.summary = "line1\nline=2\\tail\r".to_string();
        object.attributes = vec![NamedValue::new("z", "last"), NamedValue::new("a", "first")];
        object.relations = vec![NamedValue::new("member_of", "system:boos")];

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

    #[test]
    fn renders_multiple_objects_in_id_order_with_one_blank_line() {
        let rendered = render_objects(&[
            capability("capability:write-file"),
            capability("capability:help"),
        ]);
        let help_position = rendered.find("id=capability:help").unwrap();
        let write_position = rendered.find("id=capability:write-file").unwrap();

        assert!(help_position < write_position);
        assert_eq!(rendered.matches("\n\n").count(), 1);
    }
}
