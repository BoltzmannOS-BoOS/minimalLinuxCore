use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config;

/// A registered command from /etc/boos/commands/*.cmd
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub enable_flag: String,      // was "capability", now "enable_flag"
    pub description: String,
    pub exec: String,             // __builtin_* or /path/to/binary
    pub params: Vec<ParamDef>,    // declared parameters
}

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: String,
    pub required: bool,
}

/// Parse a key=value config file into a HashMap.
pub fn parse_kv_file(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let val = line[pos + 1..].trim().to_string();
                map.insert(key, val);
            }
        }
    }
    map
}

/// Load all commands from the registry directory.
pub fn load_commands() -> Vec<Command> {
    let mut commands = Vec::new();
    let dir = Path::new(config::CMD_DIR);

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return commands,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "cmd") {
            let kv = parse_kv_file(&path);

            let name = kv.get("name").cloned().unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            // Support both old key "capability" and new key "enable_flag"
            let enable_flag = kv.get("enable_flag")
                .or_else(|| kv.get("capability"))
                .cloned()
                .unwrap_or_default();

            let description = kv.get("description").cloned().unwrap_or_default();
            let exec = kv.get("exec").cloned().unwrap_or_default();

            // Parse params if present (format: "name:required,name:optional")
            let params = kv.get("params")
                .map(|p| parse_params(p))
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
    commands.sort_by(|a, b| a.name.cmp(&b.name));
    commands
}

/// Parse a params string like "id:required" or "level:optional"
fn parse_params(s: &str) -> Vec<ParamDef> {
    s.split(',')
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                return None;
            }
            let parts: Vec<&str> = p.splitn(2, ':').collect();
            let name = parts[0].trim().to_string();
            let required = parts.get(1).map_or(true, |r| r.trim() == "required");
            Some(ParamDef { name, required })
        })
        .collect()
}

/// Check if a capability enable flag is set to 1.
pub fn is_enabled(flag: &str) -> bool {
    let path = Path::new(config::CAP_FILE);
    let kv = parse_kv_file(&path);
    kv.get(flag).map(|v| v == "1").unwrap_or(false)
}

/// Find a command by name in the registry.
pub fn find_command(name: &str) -> Option<Command> {
    let commands = load_commands();
    commands.into_iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_kv_file() {
        let dir = std::env::temp_dir().join("boos-test-parse-kv");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.cmd");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "name=test").unwrap();
        writeln!(f, "capability=allow_test").unwrap();
        writeln!(f, "description=run a test").unwrap();
        writeln!(f, "exec=__builtin_test").unwrap();
        drop(f);

        let kv = parse_kv_file(&path);
        assert_eq!(kv.get("name").unwrap(), "test");
        assert_eq!(kv.get("capability").unwrap(), "allow_test");
        assert_eq!(kv.get("description").unwrap(), "run a test");
        assert_eq!(kv.get("exec").unwrap(), "__builtin_test");
        assert!(kv.get("nonexistent").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_kv_file_empty() {
        let dir = std::env::temp_dir().join("boos-test-parse-empty");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("empty.cmd");
        std::fs::File::create(&path).unwrap();

        let kv = parse_kv_file(&path);
        assert!(kv.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_kv_file_comments() {
        let dir = std::env::temp_dir().join("boos-test-parse-comments");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("comments.cmd");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# this is a comment").unwrap();
        writeln!(f, "name=test").unwrap();
        writeln!(f, "  # indented comment").unwrap();
        writeln!(f, "exec=__builtin_test").unwrap();
        drop(f);

        let kv = parse_kv_file(&path);
        assert_eq!(kv.get("name").unwrap(), "test");
        assert_eq!(kv.get("exec").unwrap(), "__builtin_test");
        // "#" at start of line is a comment, but "  #" is not a valid key=value
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_params() {
        let params = parse_params("id:required");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "id");
        assert!(params[0].required);

        let params = parse_params("level:optional");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "level");
        assert!(!params[0].required);
    }

    #[test]
    fn test_parse_params_multiple() {
        let params = parse_params("id:required,verbose:optional");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "id");
        assert!(params[0].required);
        assert_eq!(params[1].name, "verbose");
        assert!(!params[1].required);
    }

    #[test]
    fn test_parse_params_empty() {
        let params = parse_params("");
        assert!(params.is_empty());

        let params = parse_params("  ,  ");
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_params_default_required() {
        // If no ":optional" suffix, defaults to required
        let params = parse_params("command");
        assert_eq!(params.len(), 1);
        assert!(params[0].required);
    }

    #[test]
    fn test_is_enabled_nonexistent_file() {
        // is_enabled reads from CAP_FILE which doesn't exist in test
        // Should return false for nonexistent flags
        assert!(!is_enabled("nonexistent_flag_xyz"));
    }

    #[test]
    fn test_command_backward_compat() {
        // Ensure we can read .cmd file with old "capability" key
        let dir = std::env::temp_dir().join("boos-test-compat");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("old.cmd");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "name=oldcmd").unwrap();
        writeln!(f, "capability=allow_old").unwrap();
        writeln!(f, "description=old style").unwrap();
        writeln!(f, "exec=__builtin_help").unwrap();
        drop(f);

        let kv = parse_kv_file(&path);
        let enable_flag = kv.get("enable_flag")
            .or_else(|| kv.get("capability"))
            .cloned()
            .unwrap_or_default();
        assert_eq!(enable_flag, "allow_old");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
