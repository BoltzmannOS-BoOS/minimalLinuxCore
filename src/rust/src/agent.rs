//! Agent loop daemon: runs alongside gateway, manages 3-tier memory.
//!
//! The agent loop:
//!   1. Starts a session (or joins existing)
//!   2. Runs the gateway for AI interaction
//!   3. Exposes memory builtins: remember, recall, observe, session
//!   4. Periodically writes working memory to disk
//!
//! Architecture: boos-agent is a daemon that starts the gateway in-process
//! (like supervisor does) and also manages memory state. The AI interacts
//! through the gateway as usual, and memory commands are routed through
//! boos-exec like any other command.

use std::fs;
use std::process::{Command, Child};
use std::time::{Duration, Instant};

use crate::config;
use crate::log;
use crate::memory;

// ── Memory command implementations ─────────────────────────────────────────
// These are called by exec.rs when the AI submits builtin commands.

/// Handle `session start <id>` command.
pub fn cmd_session_start(args: &str) -> i32 {
    let session_id = args.trim();
    if session_id.is_empty() {
        let id = &format!("sess-{}", memory::now_secs());
        match memory::session_start(id) {
            Ok(wm) => {
                println!("Session started: {}", wm.session_id);
                config::EXIT_ALLOWED
            }
            Err(e) => {
                eprintln!("Failed to start session: {}", e);
                config::EXIT_ERROR
            }
        }
    } else {
        match memory::session_start(session_id) {
            Ok(wm) => {
                println!("Session started: {}", wm.session_id);
                config::EXIT_ALLOWED
            }
            Err(e) => {
                eprintln!("Failed to start session: {}", e);
                config::EXIT_ERROR
            }
        }
    }
}

/// Handle `session status` command.
pub fn cmd_session_status() -> i32 {
    match memory::WorkingMemory::load() {
        Ok(wm) => {
            println!("Session: {}", wm.session_id);
            println!("Goals: {}", wm.goals.join(", "));
            println!("Facts: {} active", wm.active_facts.len());
            for f in &wm.active_facts {
                println!("  - {}", f);
            }
            println!("Context: {} entries", wm.context.len());
            for (k, v) in &wm.context {
                println!("  {} = {}", k, v);
            }
            config::EXIT_ALLOWED
        }
        Err(_) => {
            println!("No active session. Use 'session start' to begin.");
            config::EXIT_ALLOWED
        }
    }
}

/// Handle `session end` command.
pub fn cmd_session_end() -> i32 {
    match memory::session_end() {
        Ok(()) => {
            println!("Session ended and archived.");
            config::EXIT_ALLOWED
        }
        Err(e) => {
            eprintln!("Failed to end session: {}", e);
            config::EXIT_ERROR
        }
    }
}

/// Handle `session goal <goal>` command.
pub fn cmd_session_goal(args: &str) -> i32 {
    let goal = args.trim();
    if goal.is_empty() {
        eprintln!("Usage: session goal <goal>");
        return config::EXIT_ERROR;
    }
    match memory::WorkingMemory::load() {
        Ok(mut wm) => {
            wm.add_goal(goal);
            if wm.save().is_ok() {
                println!("Goal added: {}", goal);
                config::EXIT_ALLOWED
            } else {
                eprintln!("Failed to save working memory");
                config::EXIT_ERROR
            }
        }
        Err(_) => {
            eprintln!("No active session. Use 'session start' first.");
            config::EXIT_ERROR
        }
    }
}

/// Handle `remember <key> <value> [tags]` command.
pub fn cmd_remember(args: &str) -> i32 {
    let args = args.trim();
    if args.is_empty() {
        eprintln!("Usage: remember <key> <value> [tags]");
        return config::EXIT_ERROR;
    }

    // Split into key + rest. Multi-word values are joined back.
    // Supports: remember key some value    → key=key, value="some value"
    //           remember key value :tag    → key=key, value="value", tags="tag"
    let space_pos = match args.find(' ') {
        Some(p) => p,
        None => {
            eprintln!("Usage: remember <key> <value> [tags]");
            return config::EXIT_ERROR;
        }
    };
    let key = args[..space_pos].trim();
    let rest = args[space_pos + 1..].trim();

    if key.is_empty() || rest.is_empty() {
        eprintln!("Key and value must not be empty");
        return config::EXIT_ERROR;
    }

    // Check for tags separator: " :tag" or " :tag1,tag2"
    let (value, tags) = if let Some(tag_pos) = rest.rfind(" :") {
        let val = rest[..tag_pos].trim();
        let t = rest[tag_pos + 2..].trim();
        (val, t)
    } else {
        (rest, "")
    };

    let session_id = load_session_id();
    match memory::archive_set(key, value, &session_id, tags) {
        Ok(()) => {
            println!("Remembered: {} = {}", key, value);
            config::EXIT_ALLOWED
        }
        Err(e) => {
            eprintln!("Failed to remember: {}", e);
            config::EXIT_ERROR
        }
    }
}

/// Handle `recall <query>` command.
pub fn cmd_recall(args: &str) -> i32 {
    let query = args.trim();

    if query == "--recent" {
        // Show recent memory entries
        let entries = memory::recent_entries();
        if entries.is_empty() {
            println!("No recent entries.");
        } else {
            println!("Recent memory ({} entries):", entries.len());
            for (_i, e) in entries.iter().rev().take(10).enumerate() {
                println!("  [{:.0}] {} {}",
                    e.ts, e.entry_type,
                    log::json_escape(&e.content));
            }
        }
        return config::EXIT_ALLOWED;
    }

    if query.starts_with("--recent ") {
        let n: usize = query["--recent ".len()..].trim().parse().unwrap_or(10);
        let entries = memory::recent_entries();
        if entries.is_empty() {
            println!("No recent entries.");
        } else {
            for e in entries.iter().rev().take(n) {
                println!("  [{:.0}] {} {}",
                    e.ts, e.entry_type,
                    log::json_escape(&e.content));
            }
        }
        return config::EXIT_ALLOWED;
    }

    if query.is_empty() {
        // List all archive entries
        let entries = memory::archive_search("");
        if entries.is_empty() {
            println!("No archived entries.");
        } else {
            println!("Archive memory ({} entries):", entries.len());
            for e in &entries {
                println!("  {} = {} (session: {}, tags: {})",
                    e.key, log::json_escape(&e.value), e.session_id, e.tags);
            }
        }
        return config::EXIT_ALLOWED;
    }

    // Search archive
    let archive_results = memory::archive_search(query);
    let recent_results = memory::recent_search(query);

    println!("Recall results for '{}':", query);
    if !archive_results.is_empty() {
        println!("  Archive:");
        for e in &archive_results {
            println!("    {} = {}", e.key, log::json_escape(&e.value));
        }
    }
    if !recent_results.is_empty() {
        println!("  Recent:");
        for e in &recent_results {
            println!("    [{}] {}", e.entry_type, log::json_escape(&e.content));
        }
    }

    if archive_results.is_empty() && recent_results.is_empty() {
        println!("  No matches found.");
    }

    config::EXIT_ALLOWED
}

/// Handle `observe <content>` command.
pub fn cmd_observe(args: &str) -> i32 {
    let content = args.trim();
    if content.is_empty() {
        eprintln!("Usage: observe <content>");
        return config::EXIT_ERROR;
    }

    let session_id = load_session_id();
    let entry = memory::RecentEntry::new("observation", content, &session_id);
    match memory::recent_add(entry) {
        Ok(()) => {
            // Also add as active fact in working memory
            if let Ok(mut wm) = memory::WorkingMemory::load() {
                wm.add_fact(content);
                let _ = wm.save();
            }
            println!("Observed.");
            config::EXIT_ALLOWED
        }
        Err(e) => {
            eprintln!("Failed to observe: {}", e);
            config::EXIT_ERROR
        }
    }
}

/// Handle `forget <key>` command.
pub fn cmd_forget(args: &str) -> i32 {
    let key = args.trim();
    if key.is_empty() {
        eprintln!("Usage: forget <key>");
        return config::EXIT_ERROR;
    }
    match memory::archive_delete(key) {
        Ok(()) => {
            println!("Forgotten: {}", key);
            config::EXIT_ALLOWED
        }
        Err(e) => {
            eprintln!("Failed to forget: {}", e);
            config::EXIT_ERROR
        }
    }
}

/// Handle `context set <key> <value>` command.
pub fn cmd_context_set(args: &str) -> i32 {
    let args = args.trim();
    if args.is_empty() {
        eprintln!("Usage: context set <key> <value>");
        return config::EXIT_ERROR;
    }
    let space_pos = match args.find(' ') {
        Some(p) => p,
        None => {
            eprintln!("Usage: context set <key> <value>");
            return config::EXIT_ERROR;
        }
    };
    let key = args[..space_pos].trim();
    let value = args[space_pos + 1..].trim();

    if key.is_empty() || value.is_empty() {
        eprintln!("Usage: context set <key> <value>");
        return config::EXIT_ERROR;
    }

    match memory::WorkingMemory::load() {
        Ok(mut wm) => {
            wm.add_context(key, value);
            if wm.save().is_ok() {
                println!("Context set: {} = {}", key, value);
                config::EXIT_ALLOWED
            } else {
                eprintln!("Failed to save working memory");
                config::EXIT_ERROR
            }
        }
        Err(_) => {
            eprintln!("No active session. Use 'session start' first.");
            config::EXIT_ERROR
        }
    }
}

/// Handle `context get <key>` command.
pub fn cmd_context_get(args: &str) -> i32 {
    let key = args.trim();
    if key.is_empty() {
        eprintln!("Usage: context get <key>");
        return config::EXIT_ERROR;
    }
    match memory::WorkingMemory::load() {
        Ok(wm) => {
            match wm.context.get(key) {
                Some(v) => {
                    println!("{}", v);
                    config::EXIT_ALLOWED
                }
                None => {
                    println!("(not set)");
                    config::EXIT_ALLOWED
                }
            }
        }
        Err(_) => {
            println!("No active session.");
            config::EXIT_ERROR
        }
    }
}

// ── Agent loop ─────────────────────────────────────────────────────────────

fn load_session_id() -> String {
    memory::WorkingMemory::load()
        .map(|wm| wm.session_id)
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Main entry for boos-agent daemon.
/// Starts the agent loop: initializes memory, runs gateway, polls for requests.
pub fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check for session subcommands first
    if args.len() >= 2 {
        match args[1].as_str() {
            "session" => {
                let rest = args.get(2).map(|s| s.as_str()).unwrap_or("");
                let rest2 = args[3..].join(" ");
                match rest {
                    "start" => {
                        std::process::exit(cmd_session_start(&rest2));
                    }
                    "status" => {
                        std::process::exit(cmd_session_status());
                    }
                    "end" => {
                        std::process::exit(cmd_session_end());
                    }
                    "goal" => {
                        std::process::exit(cmd_session_goal(&rest2));
                    }
                    _ => {
                        eprintln!("Usage: boos-agent session <start|status|end|goal>");
                        std::process::exit(config::EXIT_ERROR);
                    }
                }
            }
            _ => {}
        }
    }

    // Check for explore subcommand
    if args.len() >= 2 && args[1] == "explore" {
        let bold = args.get(2).map(|s| s == "--bold").unwrap_or(false);
        crate::explore::run_explore(bold);
        return;
    }

    // Check for loop subcommand (autonomous DeepSeek agent)
    if args.len() >= 2 && args[1] == "loop" {
        let mut api_key: Option<String> = None;
        let mut goal = "探索BoOS并了解它的能力".to_string();
        let mut max_loops = 30u32;
        let mut prior_knowledge: Option<String> = None;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--api-key" => {
                    i += 1;
                    if i < args.len() { api_key = Some(args[i].clone()); }
                }
                "--goal" => {
                    i += 1;
                    if i < args.len() { goal = args[i].clone(); }
                }
                "--max-loops" => {
                    i += 1;
                    if i < args.len() { max_loops = args[i].parse().unwrap_or(30); }
                }
                "--prior-knowledge" => {
                    i += 1;
                    if i < args.len() { prior_knowledge = Some(args[i].clone()); }
                }
                _ => {}
            }
            i += 1;
        }
        crate::agent_loop::run_loop(api_key.as_deref(), &goal, max_loops, prior_knowledge.as_deref());
        return;
    }

    // Check for develop subcommand (autonomous development agent)
    if args.len() >= 2 && args[1] == "develop" {
        let mut api_key: Option<String> = None;
        let mut goal = "改进BoOS".to_string();
        let mut max_loops = 20u32;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--api-key" => {
                    i += 1;
                    if i < args.len() { api_key = Some(args[i].clone()); }
                }
                "--goal" => {
                    i += 1;
                    if i < args.len() { goal = args[i].clone(); }
                }
                "--max-loops" => {
                    i += 1;
                    if i < args.len() { max_loops = args[i].parse().unwrap_or(20); }
                }
                _ => {}
            }
            i += 1;
        }
        crate::agent_develop::run_develop(api_key.as_deref(), &goal, max_loops);
        return;
    }

    // Daemon mode: start agent loop with gateway
    log::log("boos-agent", "started", &[("mode", "agent_loop")]);

    // Ensure memory directory exists
    let _ = fs::create_dir_all(config::MEMORY_DIR);
    let _ = fs::create_dir_all(config::MEMORY_DIR.to_string() + "/recent");
    let _ = fs::create_dir_all(config::MEMORY_DIR.to_string() + "/archive");

    // Auto-start a session if none exists
    if memory::WorkingMemory::load().is_err() {
        let id = format!("sess-{}", memory::now_secs());
        let wm = memory::WorkingMemory::new(id);
        let _ = wm.save();
    }

    // Start gateway in a child process
    let port = config::GATEWAY_DEFAULT_PORT;
    let mut gateway_child: Option<Child> = match Command::new("/bin/boos-gateway")
        .arg(port.to_string())
        .spawn()
    {
        Ok(c) => {
            log::log("boos-agent", "gateway_started", &[
                ("port", &port.to_string()),
            ]);
            Some(c)
        }
        Err(e) => {
            log::log("boos-agent", "error", &[
                ("msg", "failed to start gateway"),
                ("error", &e.to_string()),
            ]);
            eprintln!("Failed to start gateway: {}", e);
            std::process::exit(1);
        }
    };

    // Periodically persist working memory and check gateway health
    let persist_interval = Duration::from_secs(5);
    let mut last_persist = Instant::now();

    loop {
        // Persist working memory periodically
        if last_persist.elapsed() >= persist_interval {
            if let Ok(mut wm) = memory::WorkingMemory::load() {
                wm.last_updated = memory::now_secs();
                let _ = wm.save();
            }
            last_persist = Instant::now();
        }

        // Check gateway health
        if let Some(ref mut child) = gateway_child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    log::log("boos-agent", "gateway_exited", &[
                        ("status", &status.to_string()),
                    ]);
                    // Restart gateway
                    match Command::new("/bin/boos-gateway")
                        .arg(port.to_string())
                        .spawn()
                    {
                        Ok(c) => {
                            *child = c;
                            log::log("boos-agent", "gateway_restarted", &[("port", &port.to_string())]);
                        }
                        Err(e) => {
                            log::log("boos-agent", "error", &[
                                ("msg", "gateway restart failed"),
                                ("error", &e.to_string()),
                            ]);
                        }
                    }
                }
                Ok(None) => {} // Still running
                Err(e) => {
                    log::log("boos-agent", "error", &[
                        ("msg", "gateway health check failed"),
                        ("error", &e.to_string()),
                    ]);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }
}
