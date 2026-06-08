// BoOS Decision Gate — intercepts high-impact actions, auto-checkpoints
// before irreversible state changes.
// Architecture: AI proposes action → Gate intercepts → checkpoint if needed → execute → log

use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// A decision point: what was the choice, what were alternatives, what did we expect.
#[derive(Debug, Clone)]
pub struct Decision {
    pub id: String,
    pub action: String,
    pub reason: String,
    pub expected_outcome: String,
    pub reversible: bool,
    pub timestamp: u64,
    pub snapshot_path: String,
}

/// Snapshot of session state before a high-impact action.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub decision_id: String,
    pub recent_actions_count: usize,
    pub working_memory_snapshot: Vec<(String, String)>,
}

impl Decision {
    pub fn new(action: &str, reason: &str, expected: &str, reversible: bool) -> Self {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let id = format!("dp-{}-{:x}", ts, action.len().wrapping_mul(7));
        let snapshot_path = format!("/var/boos/snapshots/{}.snap", id);
        Decision {
            id, action: action.to_string(), reason: reason.to_string(),
            expected_outcome: expected.to_string(), reversible,
            timestamp: ts, snapshot_path,
        }
    }
}

/// Gate: determines if an action needs a checkpoint before execution.
pub struct DecisionGate {
    pub decisions: Vec<Decision>,
    pub snapshots_dir: String,
}

impl DecisionGate {
    pub fn new() -> Self {
        let _ = fs::create_dir_all("/var/boos/snapshots");
        DecisionGate { decisions: Vec::new(), snapshots_dir: "/var/boos/snapshots".into() }
    }

    /// Returns true if this action is high-impact (needs checkpoint).
    pub fn is_high_impact(action: &str) -> bool {
        let upper = action.to_uppercase();
        upper.starts_with("WRITE ") || upper.starts_with("FETCH ") || 
        upper.starts_with("BUILD") || upper.starts_with("TEST") ||
        upper == "DONE" || upper.starts_with("DONE ")
    }

    /// Intercept before execution: create checkpoint if high-impact.
    /// Returns the decision ID if a checkpoint was created.
    pub fn intercept_before(&mut self, action: &str, recent_actions: &[String]) -> Option<String> {
        if !Self::is_high_impact(action) { return None; }
        
        let reason = match action.chars().next().unwrap_or('?') {
            'W' | 'w' => "file mutation",
            'F' | 'f' => "external data fetch",
            'B' | 'b' => "expensive build",
            'T' | 't' => "test execution",
            'D' | 'd' => "task completion (irreversible)",
            _ => "high-impact action",
        };

        let reversible = !action.to_uppercase().starts_with("DONE");
        let expected = format!("action succeeds: {}", &action[..action.len().min(60)]);
        let d = Decision::new(action, reason, &expected, reversible);
        
        // Save snapshot: current state
        let snap = Snapshot {
            decision_id: d.id.clone(),
            recent_actions_count: recent_actions.len(),
            working_memory_snapshot: Vec::new(), // TODO: read working.kv
        };

        let snap_json = format!("{{\"decision_id\":\"{}\",\"count\":{}}}", 
            d.id, snap.recent_actions_count);
        let _ = fs::write(&d.snapshot_path, &snap_json);
        
        let id = d.id.clone();
        self.decisions.push(d);
        Some(id)
    }

    /// List recent decisions for audit.
    pub fn list(&self) -> Vec<String> {
        self.decisions.iter().rev().take(10).map(|d| {
            format!("{}: {} [reversible:{}]", d.id, &d.action[..d.action.len().min(50)], d.reversible)
        }).collect()
    }

    /// Rollback: list alternative paths from a checkpoint.
    pub fn rollback_to(&self, _decision_id: &str) -> String {
        "Rollback: checkpoint exists, but full rollback needs Branch Manager (v0.9.1)".to_string()
    }
}
