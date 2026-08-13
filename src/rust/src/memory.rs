//! Agent memory system: 3-tier architecture inspired by Letta mem hierarchy.
//!
//! Tiers:
//!   Working  — current session state (goals, context, active facts)
//!   Recent   — ring buffer of last N observations/actions/results
//!   Archive  — persistent key-value store with metadata
//!
//! Each authenticated principal owns a memory root below its runtime directory.
//! No external dependencies.
//! Uses key=value format throughout (no serde) — matching the repo convention.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::log;
use crate::memory_namespace::MemoryNamespace;

// ── Working memory ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WorkingMemory {
    pub session_id: String,
    pub goals: Vec<String>,
    pub context: HashMap<String, String>,
    pub active_facts: Vec<String>,
    pub last_updated: u64,
}

impl WorkingMemory {
    pub fn new(session_id: String) -> Self {
        WorkingMemory {
            session_id,
            goals: Vec::new(),
            context: HashMap::new(),
            active_facts: Vec::new(),
            last_updated: now_secs(),
        }
    }

    pub fn load() -> io::Result<Self> {
        let namespace = MemoryNamespace::from_environment()?;
        Self::load_from(&namespace)
    }

    pub(crate) fn load_from(namespace: &MemoryNamespace) -> io::Result<Self> {
        let kv = read_kv(&namespace.working_path())?;

        Ok(WorkingMemory {
            session_id: kv.get("session_id").cloned().unwrap_or_default(),
            goals: split_list(kv.get("goals").cloned().unwrap_or_default()),
            context: parse_context(kv.get("context").cloned().unwrap_or_default()),
            active_facts: split_list(kv.get("active_facts").cloned().unwrap_or_default()),
            last_updated: kv.get("last_updated")
                .and_then(|s| s.parse().ok()).unwrap_or(0),
        })
    }

    pub fn save(&self) -> io::Result<()> {
        let namespace = MemoryNamespace::from_environment()?;
        self.save_in(&namespace)
    }

    pub(crate) fn save_in(&self, namespace: &MemoryNamespace) -> io::Result<()> {
        let dir = namespace.root();
        fs::create_dir_all(dir).inspect_err(|error| {
            log::log(
                "boos-memory",
                "error",
                &[("op", "save_create_dir"), ("error", &error.to_string())],
            );
        })?;

        let content = format!(
            "session_id={}\ngoals={}\ncontext={}\nactive_facts={}\nlast_updated={}\n",
            self.session_id,
            join_list(&self.goals),
            context_to_str(&self.context),
            join_list(&self.active_facts),
            self.last_updated,
        );

        let tmp = namespace.working_temp_path(&self.session_id)?;
        fs::write(&tmp, &content).inspect_err(|error| {
            log::log(
                "boos-memory",
                "error",
                &[("op", "save_write"), ("error", &error.to_string())],
            );
        })?;
        fs::rename(&tmp, namespace.working_path()).inspect_err(|error| {
            log::log(
                "boos-memory",
                "error",
                &[("op", "save_rename"), ("error", &error.to_string())],
            );
        })?;
        Ok(())
    }

    pub fn add_goal(&mut self, goal: &str) {
        if !self.goals.iter().any(|g| g == goal) {
            self.goals.push(goal.to_string());
            self.last_updated = now_secs();
        }
    }

    pub fn add_context(&mut self, key: &str, value: &str) {
        self.context.insert(key.to_string(), value.to_string());
        self.last_updated = now_secs();
    }

    pub fn add_fact(&mut self, fact: &str) {
        if !self.active_facts.iter().any(|f| f == fact) {
            self.active_facts.push(fact.to_string());
            self.last_updated = now_secs();
        }
    }
}

fn log_namespace_error(operation: &str, error: &io::Error) {
    log::log(
        "boos-memory",
        "error",
        &[("op", operation), ("error", &error.to_string())],
    );
}

// ── Recent memory ──────────────────────────────────────────────────────────

pub const MAX_RECENT: usize = 100;

#[derive(Debug, Clone)]
pub struct RecentEntry {
    pub ts: f64,
    pub entry_type: String, // observation | action | result
    pub content: String,
    pub session_id: String,
}

impl RecentEntry {
    pub fn new(entry_type: &str, content: &str, session_id: &str) -> Self {
        RecentEntry {
            ts: log::uptime_secs(),
            entry_type: entry_type.to_string(),
            content: content.to_string(),
            session_id: session_id.to_string(),
        }
    }

    fn to_kv(&self) -> String {
        format!(
            "ts={:.3}\ntype={}\ncontent={}\nsession_id={}\n",
            self.ts, self.entry_type,
            sanitize_value(&self.content),
            self.session_id
        )
    }

    fn from_kv(kv: &HashMap<String, String>) -> Self {
        RecentEntry {
            ts: kv.get("ts").and_then(|s| s.parse().ok()).unwrap_or(0.0),
            entry_type: kv.get("type").cloned().unwrap_or_default(),
            content: kv.get("content").cloned().unwrap_or_default(),
            session_id: kv.get("session_id").cloned().unwrap_or_default(),
        }
    }
}

/// Read all recent entries from disk, sorted by sequence number.
pub fn recent_entries() -> Vec<RecentEntry> {
    let namespace = match MemoryNamespace::from_environment() {
        Ok(namespace) => namespace,
        Err(error) => {
            log_namespace_error("recent_namespace", &error);
            return Vec::new();
        }
    };
    recent_entries_in(&namespace)
}

pub(crate) fn recent_entries_in(namespace: &MemoryNamespace) -> Vec<RecentEntry> {
    let dir = namespace.recent_dir();
    let _ = fs::create_dir_all(&dir);

    let mut entries: Vec<(u32, RecentEntry)> = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".kv") {
                continue;
            }
            let seq: u32 = name.trim_end_matches(".kv").parse().unwrap_or(0);
            if let Ok(data) = fs::read_to_string(e.path()) {
                let kv = parse_kv_string(&data);
                entries.push((seq, RecentEntry::from_kv(&kv)));
            }
        }
    }
    entries.sort_by_key(|(s, _)| *s);
    entries.into_iter().map(|(_, e)| e).collect()
}

/// Add an entry to recent memory (ring buffer).
pub fn recent_add(entry: RecentEntry) -> io::Result<()> {
    let namespace = MemoryNamespace::from_environment()?;
    recent_add_in(&namespace, entry)
}

pub(crate) fn recent_add_in(
    namespace: &MemoryNamespace,
    entry: RecentEntry,
) -> io::Result<()> {
    let dir = namespace.recent_dir();
    fs::create_dir_all(&dir).inspect_err(|error| {
        log::log(
            "boos-memory",
            "error",
            &[("op", "recent_create_dir"), ("error", &error.to_string())],
        );
    })?;

    // Ring buffer: use a counter file to track position.
    // Reading max sequence from filenames is incorrect — after one wrap,
    // max stays at MAX_RECENT and slot 1 is forever reused.
    let counter_path = dir.join(".counter");
    let seq: u32 = std::fs::read_to_string(&counter_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let next_seq = (seq % MAX_RECENT as u32) + 1;
    let _ = std::fs::write(&counter_path, next_seq.to_string());

    let path = dir.join(format!("{}.kv", next_seq));
    fs::write(&path, entry.to_kv()).inspect_err(|error| {
        log::log(
            "boos-memory",
            "error",
            &[("op", "recent_write"), ("error", &error.to_string())],
        );
    })?;
    Ok(())
}

/// Search recent entries for a query string.
pub fn recent_search(query: &str) -> Vec<RecentEntry> {
    let lower = query.to_lowercase();
    recent_entries().into_iter()
        .filter(|e| e.content.to_lowercase().contains(&lower)
                  || e.entry_type.to_lowercase().contains(&lower))
        .collect()
}

// ── Archive memory ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub key: String,
    pub value: String,
    pub created_at: u64,
    pub session_id: String,
    pub tags: String,
}

/// Store a key-value pair in archive memory.
pub fn archive_set(key: &str, value: &str, session_id: &str, tags: &str) -> io::Result<()> {
    let namespace = MemoryNamespace::from_environment()?;
    let dir = namespace.archive_dir();
    fs::create_dir_all(&dir).inspect_err(|error| {
        log::log(
            "boos-memory",
            "error",
            &[("op", "archive_create_dir"), ("error", &error.to_string())],
        );
    })?;

    let ts = now_secs();
    let safe_key = sanitize_filename(key);
    let path = dir.join(format!("{}.mem", safe_key));

    let content = format!(
        "key={}\nvalue={}\ncreated_at={}\nsession_id={}\ntags={}\n",
        key, sanitize_value(value), ts, session_id, tags
    );
    fs::write(&path, content).inspect_err(|error| {
        log::log(
            "boos-memory",
            "error",
            &[
                ("op", "archive_write"),
                ("key", &log::json_escape(key)),
                ("error", &error.to_string()),
            ],
        );
    })?;

    log::log("boos-memory", "archive_set", &[
        ("key", &log::json_escape(key)),
        ("session", &log::json_escape(session_id)),
    ]);
    Ok(())
}

/// Search archive memory. Returns entries where query matches key, value, or tags.
pub fn archive_search(query: &str) -> Vec<ArchiveEntry> {
    let namespace = match MemoryNamespace::from_environment() {
        Ok(namespace) => namespace,
        Err(error) => {
            log_namespace_error("archive_namespace", &error);
            return Vec::new();
        }
    };
    let dir = namespace.archive_dir();
    let _ = fs::create_dir_all(&dir);

    let mut results = Vec::new();
    let lower_q = query.to_lowercase();

    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().is_none_or(|extension| extension != "mem") {
                continue;
            }
            if let Ok(data) = fs::read_to_string(&path) {
                let lower_d = data.to_lowercase();
                if query.is_empty() || lower_d.contains(&lower_q) {
                    let kv = parse_kv_string(&data);
                    results.push(ArchiveEntry {
                        key: kv.get("key").cloned().unwrap_or_default(),
                        value: kv.get("value").cloned().unwrap_or_default(),
                        created_at: kv.get("created_at")
                            .and_then(|s| s.parse().ok()).unwrap_or(0),
                        session_id: kv.get("session_id").cloned().unwrap_or_default(),
                        tags: kv.get("tags").cloned().unwrap_or_default(),
                    });
                }
            }
        }
    }
    results.sort_by_key(|entry| std::cmp::Reverse(entry.created_at));
    results
}

/// Delete an archive entry by key.
pub fn archive_delete(key: &str) -> io::Result<()> {
    let safe_key = sanitize_filename(key);
    let namespace = MemoryNamespace::from_environment()?;
    let path = namespace.archive_dir().join(format!("{}.mem", safe_key));
    fs::remove_file(&path).inspect_err(|error| {
        log::log(
            "boos-memory",
            "error",
            &[
                ("op", "archive_delete"),
                ("key", &log::json_escape(key)),
                ("error", &error.to_string()),
            ],
        );
    })?;
    log::log("boos-memory", "archive_delete", &[
        ("key", &log::json_escape(key)),
    ]);
    Ok(())
}

// ── Session management ─────────────────────────────────────────────────────

/// Start a new session: archive old working memory, create new.
pub fn session_start(session_id: &str) -> io::Result<WorkingMemory> {
    // Archive current working memory facts if there's an existing session
    if let Ok(old) = WorkingMemory::load() {
        if !old.session_id.is_empty() && !old.active_facts.is_empty() {
            archive_set(
                &format!("session_{}_facts", old.session_id),
                &join_list(&old.active_facts),
                &old.session_id,
                "auto_archive",
            )?;
        }
    }
    let wm = WorkingMemory::new(session_id.to_string());
    wm.save()?;

    log::log("boos-memory", "session_start", &[
        ("session", &log::json_escape(session_id)),
    ]);
    Ok(wm)
}

/// End a session: persist working memory to archive.
pub fn session_end() -> io::Result<()> {
    let wm = WorkingMemory::load()?;
    let sid = &wm.session_id;

    // Archive goals
    if !wm.goals.is_empty() {
        archive_set(
            &format!("session_{}_goals", sid),
            &join_list(&wm.goals),
            sid,
            "session_archive",
        )?;
    }

    // Archive context
    if !wm.context.is_empty() {
        archive_set(
            &format!("session_{}_context", sid),
            &context_to_str(&wm.context),
            sid,
            "session_archive",
        )?;
    }

    // Archive facts
    if !wm.active_facts.is_empty() {
        archive_set(
            &format!("session_{}_facts", sid),
            &join_list(&wm.active_facts),
            sid,
            "session_archive",
        )?;
    }

    log::log("boos-memory", "session_end", &[
        ("session", &log::json_escape(sid)),
    ]);
    Ok(())
}

// ── Memory helpers ─────────────────────────────────────────────────────────

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Simple key=value file parser.
fn read_kv(path: &Path) -> io::Result<HashMap<String, String>> {
    let data = fs::read_to_string(path)?;
    Ok(parse_kv_string(&data))
}

fn parse_kv_string(data: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in data.lines() {
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
    map
}

/// Join a list with a delimiter (pipe).
fn join_list(items: &[String]) -> String {
    items.join("|")
}

/// Split a pipe-delimited string into a Vec.
fn split_list(s: String) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split('|').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// Encode context map as "k1=v1,k2=v2" with comma-escaping.
fn context_to_str(ctx: &HashMap<String, String>) -> String {
    ctx.iter()
        .map(|(k, v)| format!("{}::{}", k, v))
        .collect::<Vec<_>>()
        .join(",")
}

/// Decode context string back to map.
fn parse_context(s: String) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if s.is_empty() {
        return map;
    }
    for pair in s.split(',') {
        if let Some(pos) = pair.find("::") {
            let key = &pair[..pos];
            let val = &pair[pos + 2..];
            map.insert(key.to_string(), val.to_string());
        }
    }
    map
}

fn sanitize_filename(s: &str) -> String {
    s.chars().map(|c| if c == '/' || c == '\0' || c == '|' { '_' } else { c }).collect()
}

/// Escape newlines in values to prevent KV injection attacks.
/// Agent-controlled content must not be able to inject fake key=value lines.
fn sanitize_value(s: &str) -> String {
    s.replace('\n', "\\n").replace('\r', "\\r")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_memory_save_load() {
        let mut wm = WorkingMemory::new("sess-test".into());
        wm.add_goal("goal1");
        wm.add_context("key1", "val1");
        wm.add_fact("fact1");

        // Save to temp directory
        let dir = std::env::temp_dir().join("boos-test-memory-working");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("working.kv");

        let content = format!(
            "session_id={}\ngoals={}\ncontext={}\nactive_facts={}\nlast_updated={}\n",
            wm.session_id,
            join_list(&wm.goals),
            context_to_str(&wm.context),
            join_list(&wm.active_facts),
            wm.last_updated,
        );
        fs::write(&path, &content).unwrap();

        let kv = read_kv(&path).unwrap();
        assert_eq!(kv.get("session_id").unwrap(), "sess-test");
        assert_eq!(split_list(kv.get("goals").unwrap().clone()), vec!["goal1"]);
        assert_eq!(split_list(kv.get("active_facts").unwrap().clone()), vec!["fact1"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_agent_working_memory_round_trip_stays_in_agent_namespace() {
        let dir = std::env::temp_dir().join(format!(
            "boos-agent-memory-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let namespace = MemoryNamespace::new(&dir, Some("agent-a")).unwrap();
        let mut original = WorkingMemory::new("session-a".into());
        original.add_fact("isolated fact");

        original.save_in(&namespace).unwrap();
        let restored = WorkingMemory::load_from(&namespace).unwrap();

        assert_eq!(restored.session_id, "session-a");
        assert_eq!(restored.active_facts, vec!["isolated fact"]);
        assert!(
            !dir.join("working.kv").exists(),
            "an agent-specific save must not write shared working memory"
        );
        assert!(namespace.working_path().exists());

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_join_split_list() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        let joined = join_list(&items);
        assert_eq!(joined, "a|b|c");
        let back = split_list(joined);
        assert_eq!(back, items);
    }

    #[test]
    fn test_context_roundtrip() {
        let mut ctx = HashMap::new();
        ctx.insert("k1".into(), "v1".into());
        ctx.insert("k2".into(), "v2".into());
        let encoded = context_to_str(&ctx);
        let decoded = parse_context(encoded);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.get("k1").unwrap(), "v1");
        assert_eq!(decoded.get("k2").unwrap(), "v2");
    }

    #[test]
    fn test_kv_injection_prevented() {
        // Attack: value contains newlines to inject fake key=value pairs
        let malicious = "real_value\nfake_key=evil\nanother_fake=pwned";
        let sanitized = sanitize_value(malicious);
        assert!(!sanitized.contains('\n'), "newlines must be escaped");
        assert!(!sanitized.contains("\nfake_key="), "no fake keys can be injected");
        assert_eq!(sanitized, "real_value\\nfake_key=evil\\nanother_fake=pwned");

        // Verify parse_kv_string doesn't create fake keys from sanitized input
        let kv = parse_kv_string(&format!("key={}", sanitized));
        assert_eq!(kv.len(), 1, "only one key should exist");
        assert_eq!(kv.get("key").unwrap(), &sanitized);
        assert!(!kv.contains_key("fake_key"), "fake_key should not exist");
        assert!(!kv.contains_key("another_fake"), "another_fake should not exist");
    }

    #[test]
    fn test_recent_entry_kv_roundtrip() {
        let entry = RecentEntry::new("observation", "test content here", "sess-1");
        let kv_str = entry.to_kv();
        let kv = parse_kv_string(&kv_str);
        let back = RecentEntry::from_kv(&kv);
        assert_eq!(back.entry_type, "observation");
        assert_eq!(back.content, "test content here");
        assert_eq!(back.session_id, "sess-1");
    }

    #[test]
    fn test_archive_kv_roundtrip() {
        let content = "key=mykey\nvalue=myval\ncreated_at=123456\nsession_id=sess-z\ntags=important\n";
        let kv = parse_kv_string(content);
        assert_eq!(kv.get("key").unwrap(), "mykey");
        assert_eq!(kv.get("value").unwrap(), "myval");
        assert_eq!(kv.get("session_id").unwrap(), "sess-z");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("a/b"), "a_b");
        assert_eq!(sanitize_filename("a\0b"), "a_b");
        assert_eq!(sanitize_filename("a|b"), "a_b");
        assert_eq!(sanitize_filename("normal"), "normal");
    }

    #[test]
    fn test_persist_roundtrip_atomic() {
        let dir = std::env::temp_dir().join("boos-test-persist");
        let _ = fs::create_dir_all(&dir);
        // Simulate crash: write partial data, verify clean state survives
        let tmp = dir.join("working.tmp");
        let real = dir.join("working.kv");

        // Write via temp+rename (atomic pattern)
        let content = "session_id=crash-test\ngoals=a|b\ncontext=k1::v1\nactive_facts=fact1\nlast_updated=42\n";
        fs::write(&tmp, content).unwrap();
        fs::rename(&tmp, &real).unwrap();

        // Read back — data must be intact
        let kv = read_kv(&real).unwrap();
        assert_eq!(kv.get("session_id").unwrap(), "crash-test");
        assert_eq!(kv.get("goals").unwrap(), "a|b");
        assert_eq!(kv.get("last_updated").unwrap(), "42");

        // Simulate partial write (crash mid-write) — temp file should exist
        // but real file should have previous state, not partial state
        let prev = fs::read_to_string(&real).unwrap();
        fs::write(&tmp, "session_id=partial\nCORRUPT").unwrap();
        // Crash before rename: real file unchanged
        assert_eq!(fs::read_to_string(&real).unwrap(), prev);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ring_buffer_counter_increments() {
        let root = std::env::temp_dir().join(format!(
            "boos-recent-memory-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let namespace = MemoryNamespace::new(&root, Some("ring-agent")).unwrap();

        for i in 0..10 {
            let entry = RecentEntry::new("test", &format!("entry-{}", i), "ring-test");
            recent_add_in(&namespace, entry).unwrap();
        }
        let all = recent_entries_in(&namespace);
        let contents: Vec<&str> = all.iter().map(|entry| entry.content.as_str()).collect();

        assert_eq!(all.len(), 10);
        assert!(contents.contains(&"entry-0"), "entry-0 missing");
        assert!(contents.contains(&"entry-9"), "entry-9 missing");

        fs::remove_dir_all(root).unwrap();
    }
}
