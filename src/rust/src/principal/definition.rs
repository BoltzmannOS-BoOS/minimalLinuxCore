use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;

use crate::config;

use super::{invalid_data, PrincipalDefinition, PrincipalId};

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
    let mut seen_uids = HashSet::new();
    for path in paths {
        let fields = parse_definition_fields(&fs::read_to_string(&path)?)?;
        let definition = definition_from_fields(&fields)?;
        if !seen_ids.insert(definition.id.clone()) {
            return Err(invalid_data("duplicate principal ID"));
        }
        if !seen_uids.insert(definition.uid) {
            return Err(invalid_data("duplicate principal UID"));
        }
        definitions.push(definition);
    }
    definitions.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(definitions)
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
            return Err(invalid_data(
                "principal definition has an invalid duplicate key",
            ));
        }
    }
    Ok(fields)
}

fn definition_from_fields(
    fields: &HashMap<String, String>,
) -> io::Result<PrincipalDefinition> {
    for field in fields.keys() {
        if !matches!(
            field.as_str(),
            "id" | "user" | "uid" | "gid" | "enabled"
        ) {
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
    let gid = required_field(fields, "gid")?
        .parse::<u32>()
        .map_err(|_| invalid_data("principal GID is invalid"))?;
    let enabled = match required_field(fields, "enabled")? {
        "0" => false,
        "1" => true,
        _ => return Err(invalid_data("principal enabled flag must be 0 or 1")),
    };

    Ok(PrincipalDefinition {
        id,
        user: user.to_string(),
        uid,
        gid,
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
