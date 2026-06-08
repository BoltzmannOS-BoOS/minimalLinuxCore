// BoOS Checkpoint System — Agent Git MVP
// Save/restore conversation state snapshots. Agent can create, list, branch, rollback.
// Cannot delete checkpoints (IMMUTABLE_DENY protection).

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub timestamp: u64,
    pub label: String,
    pub recent_actions: Vec<String>,
    pub round: u32,
    pub parent_id: Option<String>,
    pub branch_name: String,
}

impl Checkpoint {
    pub fn to_json(&self) -> String {
        let actions_json: Vec<String> = self.recent_actions.iter()
            .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect();
        format!(
            "{{\"id\":\"{}\",\"session_id\":\"{}\",\"timestamp\":{},\"label\":\"{}\",\"round\":{},\"branch\":\"{}\",\"parent\":{},\"actions\":[{}]}}",
            self.id, self.session_id, self.timestamp, self.label, self.round, self.branch_name,
            match &self.parent_id { Some(p) => format!("\"{}\"", p), None => "null".to_string() },
            actions_json.join(",")
        )
    }

    pub fn from_json(json: &str) -> Option<Checkpoint> {
        let id = extract_str(json, "\"id\":\"")?;
        let session_id = extract_str(json, "\"session_id\":\"")?;
        let timestamp = extract_u64(json, "\"timestamp\":")?;
        let label = extract_str(json, "\"label\":\"")?;
        let round = extract_u32(json, "\"round\":")?;
        let branch = extract_str(json, "\"branch\":\"").unwrap_or_else(|| "main".to_string());
        let parent = if json.contains("\"parent\":null") { None } else { extract_str(json, "\"parent\":\"").filter(|s| !s.is_empty()) };
        let actions = if let Some(start) = json.find("\"actions\":[") {
            let end = json[start..].find(']')? + start;
            let arr = &json[start + 11..end];
            arr.split(",\"")
                .map(|s| s.trim_matches('"').to_string())
                .collect()
        } else { Vec::new() };
        Some(Checkpoint { id, session_id, timestamp, label, recent_actions: actions, round, parent_id: parent, branch_name: branch })
    }
}

fn extract_str(json: &str, key: &str) -> Option<String> {
    let pos = json.find(key)?;
    let start = pos + key.len();
    let end = json[start..].find('"')?;
    Some(json[start..start+end].to_string())
}

fn extract_u64(json: &str, key: &str) -> Option<u64> {
    let pos = json.find(key)?;
    let val: String = json[pos + key.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
    val.parse().ok()
}

fn extract_u32(json: &str, key: &str) -> Option<u32> {
    extract_u64(json, key).map(|v| v as u32)
}

pub struct CheckpointManager {
    dir: PathBuf,
}

impl CheckpointManager {
    pub fn new() -> Self {
        let dir = PathBuf::from("/tmp/boos-checkpoints");
        let _ = fs::create_dir_all(&dir);
        CheckpointManager { dir }
    }

    pub fn create(&self, session_id: &str, label: &str, actions: &[String], round: u32, parent: Option<&str>) -> String {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let id = format!("ck-{}-{:x}", ts, actions.len());
        let ck = Checkpoint {
            id: id.clone(), session_id: session_id.to_string(), timestamp: ts,
            label: label.to_string(), recent_actions: actions.to_vec(), round,
            parent_id: parent.map(|s| s.to_string()), branch_name: "main".to_string(),
        };
        let path = self.dir.join(format!("{}.json", id));
        let _ = fs::write(&path, ck.to_json());
        id
    }

    pub fn list(&self) -> Vec<Checkpoint> {
        let mut result = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for e in entries.filter_map(|e| e.ok()) {
                if let Ok(content) = fs::read_to_string(e.path()) {
                    if let Some(ck) = Checkpoint::from_json(&content) {
                        result.push(ck);
                    }
                }
            }
        }
        result.sort_by_key(|c| c.timestamp);
        result
    }

    pub fn load(&self, id: &str) -> Option<Checkpoint> {
        let path = self.dir.join(format!("{}.json", id));
        fs::read_to_string(&path).ok().and_then(|s| Checkpoint::from_json(&s))
    }

    pub fn branch(&self, checkpoint_id: &str, branch_name: &str) -> Option<String> {
        let ck = self.load(checkpoint_id)?;
        let branch_id = format!("{}-{}", ck.id, branch_name);
        let path = self.dir.join(format!("{}.json", branch_id));
        let mut branch_ck = ck;
        branch_ck.id = branch_id.clone();
        branch_ck.branch_name = branch_name.to_string();
        branch_ck.parent_id = Some(checkpoint_id.to_string());
        let _ = fs::write(&path, branch_ck.to_json());
        Some(branch_id)
    }
}
