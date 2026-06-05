# Direction C: Agent-Native Control Plane — Implementation Plan

> **For Hermes:** Implement task-by-task. Commit after each task.

**Goal:** Give the agent the ability to query its own audit trail, understand failure patterns, and make decisions based on history. Close the feedback loop: intent → request → cap → execute → result → audit → intent.

**Architecture:** New `audit` command + structured query primitives. The agent can answer "what did I do last session?", "why did X fail?", "what capabilities are blocked?" without reading raw log files.

**Tech Stack:** Rust std — no new dependencies.

---

## Motivation

Current state:
```
agent submits intent → system executes → result file created
agent reads result → next action
```

Missing:
```
agent queries history → understands failure patterns → adjusts behavior
```

The existing `log`, `results`, `result <id>` commands are human-facing. We need agent-facing queries:
- `audit recent` — last N actions across sessions
- `audit failures` — all denied/errored actions  
- `audit session <id>` — all actions in a session
- `audit summary` — success/failure/denial counts

---

## Task 1: Register `audit` command in registry

**Objective:** Add audit.cmd and capability flag.

**Files:**
- Create: `rootfs/etc/boos/commands/audit.cmd`
- Modify: `rootfs/etc/boos/capabilities.conf` — add `allow_audit=1`

**Step 1: Create audit.cmd**

```
name=audit
enable_flag=allow_audit
description=query agent audit trail
exec=__builtin_audit
params=subcommand:required
```

**Step 2: Add capability flag**

Append to capabilities.conf:
```
allow_audit=1
```

**Step 3: Commit**

```bash
git add rootfs/etc/boos/commands/audit.cmd rootfs/etc/boos/capabilities.conf
git commit -m "feat: register audit command with capability flag"
```

---

## Task 2: Implement audit builtin in exec.rs

**Objective:** Add `__builtin_audit` handler that supports subcommands: `recent`, `failures`, `session`, `summary`.

**Files:**
- Modify: `src/rust/src/exec.rs` — add builtin + help text

**Step 1: Add help text**

In show_help(), after the file operations section:
```rust
    println!("  ── Agent Audit ──");
    println!("  audit recent [n]              show last N actions");
    println!("  audit failures                show denied/errored actions");
    println!("  audit session <id>            show actions in session");
    println!("  audit summary                 show action counts");
```

**Step 2: Add builtin handler in run_builtin**

```rust
        "__builtin_audit" => audit_cmd(args),
```

**Step 3: Implement audit_cmd function**

```rust
fn audit_cmd(args: &str) -> i32 {
    let args = args.trim();
    if args.is_empty() {
        eprintln!("Usage: audit <recent|failures|session|summary> [args...]");
        return EXIT_ERROR;
    }

    let (subcmd, rest) = match args.find(|c: char| c.is_whitespace()) {
        Some(pos) => (&args[..pos], args[pos..].trim()),
        None => (args, ""),
    };

    match subcmd {
        "recent" => audit_recent(rest),
        "failures" => audit_failures(rest),
        "session" => audit_session(rest),
        "summary" => audit_summary(),
        _ => {
            eprintln!("Unknown audit subcommand: {}", subcmd);
            eprintln!("Usage: audit <recent|failures|session|summary>");
            EXIT_ERROR
        }
    }
}
```

**Step 4: Implement sub-functions**

```rust
fn audit_recent(args: &str) -> i32 {
    let n: usize = args.split_whitespace().next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let mut entries: Vec<(String, std::fs::Metadata)> = Vec::new();
    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                if let Ok(meta) = path.metadata() {
                    entries.push((path.to_string_lossy().to_string(), meta));
                }
            }
        }
    }
    entries.sort_by(|a, b| b.1.modified().cmp(&a.1.modified())); // newest first

    println!("Recent {} actions:", n.min(entries.len()));
    for (path, _) in entries.iter().take(n) {
        let kv = registry::parse_kv_file(std::path::Path::new(path));
        let id = kv.get("id").map(|s| s.as_str()).unwrap_or("?");
        let cmd = kv.get("command").map(|s| s.as_str()).unwrap_or("?");
        let args = kv.get("args").map(|s| s.as_str()).unwrap_or("");
        let verdict = kv.get("verdict").map(|s| s.as_str()).unwrap_or("?");
        let session = kv.get("session_id").unwrap_or(&String::new());
        if args.is_empty() {
            println!("  {} {} -> {} (session: {})", id, cmd, verdict, session);
        } else {
            println!("  {} {} {} -> {} (session: {})", id, cmd, args, verdict, session);
        }
    }
    EXIT_ALLOWED
}

fn audit_failures(_args: &str) -> i32 {
    println!("Denied and errored actions:");
    let mut found = false;
    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                let kv = registry::parse_kv_file(&path);
                let verdict = kv.get("verdict").map(|s| s.as_str()).unwrap_or("");
                if verdict == "denied" || verdict == "error" || verdict == "unknown" {
                    let id = kv.get("id").map(|s| s.as_str()).unwrap_or("?");
                    let cmd = kv.get("command").map(|s| s.as_str()).unwrap_or("?");
                    let exit_code = kv.get("exit_code").map(|s| s.as_str()).unwrap_or("?");
                    println!("  {} {} -> {} (exit={})", id, cmd, verdict, exit_code);
                    found = true;
                }
            }
        }
    }
    if !found {
        println!("  (no failures)");
    }
    EXIT_ALLOWED
}

fn audit_session(session_id: &str) -> i32 {
    if session_id.is_empty() {
        eprintln!("Usage: audit session <session-id>");
        return EXIT_ERROR;
    }
    println!("Session: {}", session_id);
    let mut found = false;
    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                let kv = registry::parse_kv_file(&path);
                let sid = kv.get("session_id").map(|s| s.as_str()).unwrap_or("");
                if sid == session_id {
                    let id = kv.get("id").map(|s| s.as_str()).unwrap_or("?");
                    let cmd = kv.get("command").map(|s| s.as_str()).unwrap_or("?");
                    let verdict = kv.get("verdict").map(|s| s.as_str()).unwrap_or("?");
                    let exit_code = kv.get("exit_code").map(|s| s.as_str()).unwrap_or("?");
                    println!("  {} {} -> {} (exit={})", id, cmd, verdict, exit_code);
                    found = true;
                }
            }
        }
    }
    if !found {
        println!("  (no actions in this session)");
    }
    EXIT_ALLOWED
}

fn audit_summary() -> i32 {
    let mut total = 0u32;
    let mut allowed = 0u32;
    let mut denied = 0u32;
    let mut error = 0u32;
    let mut unknown = 0u32;

    if let Ok(dir) = std::fs::read_dir(config::RESULT_DIR) {
        for e in dir.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                let kv = registry::parse_kv_file(&path);
                total += 1;
                match kv.get("verdict").map(|s| s.as_str()).unwrap_or("") {
                    "allowed" => allowed += 1,
                    "denied" => denied += 1,
                    "error" => error += 1,
                    "unknown" => unknown += 1,
                    _ => {}
                }
            }
        }
    }

    println!("Audit Summary:");
    println!("  Total actions: {}", total);
    println!("  Allowed:       {}", allowed);
    println!("  Denied:        {}", denied);
    println!("  Errors:        {}", error);
    println!("  Unknown:       {}", unknown);

    if total > 0 {
        let pct = (allowed as f64 / total as f64) * 100.0;
        println!("  Success rate:  {:.1}%", pct);
    }

    EXIT_ALLOWED
}
```

**Step 4: Commit**

```bash
git add src/rust/src/exec.rs
git commit -m "feat: implement audit command — recent, failures, session, summary"
```

---

## Task 3: Extend develop agent to use audit for context

**Objective:** The develop agent's context builder includes audit summary so the agent knows what's been failing.

**Files:**
- Modify: `src/rust/src/agent_develop.rs` — update build_develop_context

**Step 1: Add audit summary to context**

In `build_develop_context()`, after the recent actions section:

```rust
    // Include audit summary so the agent knows its failure patterns
    ctx.push_str("\nAudit trail:\n");
    // Read existing results to compute quick summary
    let result_dir = "/var/boos/results";
    if let Ok(entries) = std::fs::read_dir(result_dir) {
        let mut total = 0;
        let mut failures = 0;
        for e in entries.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map_or(false, |ext| ext == "out") {
                let kv = crate::registry::parse_kv_file(&path);
                total += 1;
                let v = kv.get("verdict").map(|s| s.as_str()).unwrap_or("");
                if v == "denied" || v == "error" || v == "unknown" {
                    failures += 1;
                }
            }
        }
        ctx.push_str(&format!("  Total past actions: {}, Failures: {}\n", total, failures));
    }
```

**Step 2: Commit**

```bash
git add src/rust/src/agent_develop.rs
git commit -m "feat: develop agent includes audit summary in context"
```

---

## Task 4: Build, test, end-to-end verification

**Objective:** Verify everything compiles, tests pass, end-to-end works.

**Step 1: Build**

```bash
cd src/rust && cargo build
```

**Step 2: Tests**

```bash
cargo test
```

**Step 3: End-to-end: run develop agent that queries audit first**

```bash
boos-agent develop --goal "先 audit summary 看历史，再 audit recent 看最近5条，然后 DONE" --max-loops 6
```

**Step 4: Commit any fixes**

---

## Verification Checklist

- [ ] `audit recent` returns last N actions sorted by time
- [ ] `audit failures` filters to denied/error/unknown only
- [ ] `audit session <id>` filters by session_id
- [ ] `audit summary` shows counts + success rate
- [ ] `audit` with no args shows usage
- [ ] `audit unknown_subcmd` shows error
- [ ] Develop agent context includes audit summary
- [ ] All unit tests pass (41+)
- [ ] `commands` shows audit in registry
- [ ] End-to-end: agent uses audit to understand history
