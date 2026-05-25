# Graph Report - .  (2026-05-26)

## Corpus Check
- Corpus is ~13,869 words - fits in a single context window. You may not need a graph.

## Summary
- 152 nodes · 219 edges · 22 communities (12 shown, 10 thin omitted)
- Extraction: 87% EXTRACTED · 13% INFERRED · 0% AMBIGUOUS · INFERRED: 29 edges (avg confidence: 0.89)
- Token cost: 86,000 input · 46,879 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Logging Subsystem|Logging Subsystem]]
- [[_COMMUNITY_AI Client & Boot Sequence|AI Client & Boot Sequence]]
- [[_COMMUNITY_AI Client Python Module|AI Client Python Module]]
- [[_COMMUNITY_Command Registry|Command Registry]]
- [[_COMMUNITY_Gateway & Submit|Gateway & Submit]]
- [[_COMMUNITY_Executor Builtins|Executor Builtins]]
- [[_COMMUNITY_Capability System|Capability System]]
- [[_COMMUNITY_Process Manager|Process Manager]]
- [[_COMMUNITY_Verify Script|Verify Script]]
- [[_COMMUNITY_Core Design Philosophy|Core Design Philosophy]]
- [[_COMMUNITY_Build & Run Scripts|Build & Run Scripts]]
- [[_COMMUNITY_build-rootfs.sh|build-rootfs.sh]]
- [[_COMMUNITY_build-rust.sh|build-rust.sh]]
- [[_COMMUNITY_run-qemu.sh|run-qemu.sh]]
- [[_COMMUNITY_Verification|Verification]]
- [[_COMMUNITY_Capability System Concept|Capability System Concept]]
- [[_COMMUNITY_Persistent Overlay|Persistent Overlay]]
- [[_COMMUNITY_Debug Observability|Debug Observability]]
- [[_COMMUNITY_Config Paths|Config Paths]]
- [[_COMMUNITY_Log Event|Log Event]]
- [[_COMMUNITY_DeepSeek Repetition Bug|DeepSeek Repetition Bug]]

## God Nodes (most connected - your core abstractions)
1. `run_builtin()` - 15 edges
2. `run_conversation()` - 11 edges
3. `main()` - 11 edges
4. `main()` - 10 edges
5. `parse_kv_file()` - 10 edges
6. `main()` - 8 edges
7. `log_event()` - 7 edges
8. `Conversation Loop` - 7 edges
9. `check_enabled()` - 6 edges
10. `load_commands()` - 6 edges

## Surprising Connections (you probably didn't know these)
- `Capability System (allow_*=0|1)` --references--> `check_enabled()`  [INFERRED]
  .claude/skills/boos-dev.md → src/rust/src/exec.rs
- `Two Execution Paths Architecture` --references--> `main()`  [INFERRED]
  .claude/skills/boos-dev.md → src/rust/src/exec.rs
- `File-Based IPC` --references--> `parse_kv_file()`  [INFERRED]
  .claude/skills/boos-dev.md → src/rust/src/registry.rs
- `Command Registry Pattern` --references--> `load_commands()`  [INFERRED]
  .claude/skills/boos-dev.md → src/rust/src/registry.rs
- `Two Execution Paths Architecture` --references--> `main()`  [INFERRED]
  .claude/skills/boos-dev.md → src/rust/src/process.rs

## Hyperedges (group relationships)
- **System Boot Sequence** — rootfs_init_init_script, bin_boos_supervisor_supervisor, bin_boos_shell_shell [EXTRACTED 1.00]
- **Request Pipeline** — bin_mock_ai_driver_driver, bin_boos_daemon_daemon, ai_client_conversation_loop, plan_request_queue_concept [INFERRED 0.85]
- **Rust Core Components** — plan_boos_exec, plan_boos_process, plan_boos_submit, plan_boos_gateway [EXTRACTED 1.00]
- **Request Lifecycle Pipeline** — src_submit_main, src_process_main, src_exec_main, src_exec_run_builtin, src_exec_check_enabled, src_gateway_handle_connection [EXTRACTED 1.00]
- **Registry and Capability System** — src_registry_command, src_registry_load_commands, src_registry_find_command, src_registry_is_enabled, src_registry_parse_kv_file, src_exec_check_enabled, boos_command_registry_pattern, boos_capability_system [EXTRACTED 1.00]
- **Logging Subsystem** — src_log_write_log_bytes, src_log_event, src_log_allowed, src_log_denied, src_log_append_log_line, src_log_maybe_rotate_log, src_log_get_trace_level, src_log_tracelevel [EXTRACTED 1.00]

## Communities (22 total, 10 thin omitted)

### Community 0 - "Logging Subsystem"
Cohesion: 0.14
Nodes (15): Log Rotation and Pruning (A5), Limits and concurrency constants (MAX_OUTPUT_BYTES, MAX_LOG_LINE_LEN, MAX_GATEWAY_THREADS), append_log_line(), get_trace_level(), json_escape(), log(), log_allowed(), log_denied() (+7 more)

### Community 1 - "AI Client & Boot Sequence"
Cohesion: 0.10
Nodes (21): BoOS AI Client, Command Registry Loader, Conversation Loop, Request Writer, Result Poller, OpenAI Tool Builder, BoOS Daemon, BoOS Shell (+13 more)

### Community 2 - "AI Client Python Module"
Cohesion: 0.15
Nodes (19): build_command_list_text(), build_tools(), cleanup_old_files(), ensure_dirs(), format_tool_result(), load_command_registry(), main(), make_client() (+11 more)

### Community 3 - "Command Registry"
Cohesion: 0.19
Nodes (16): Command Registry Pattern, File-Based IPC, Command, find_command(), load_commands(), ParamDef, parse_kv_file(), parse_params() (+8 more)

### Community 4 - "Gateway & Submit"
Cohesion: 0.19
Nodes (13): Two Execution Paths Architecture, Atomic Write Pattern, Gateway Concurrency Model (A1), get_auth_token(), handle_connection(), main(), main(), generate_id() (+5 more)

### Community 5 - "Executor Builtins"
Cohesion: 0.25
Nodes (14): list_commands(), main(), prune_results(), rotate_logs_cmd(), run_builtin(), set_debug(), show_caps(), show_debug() (+6 more)

### Community 6 - "Capability System"
Cohesion: 0.33
Nodes (7): Capability System (allow_*=0|1), M15 Issues Audit Report, Exit Code Contract (B8), Exit code constants (EXIT_ALLOWED/DENIED/ERROR/UNKNOWN), check_enabled(), log_denied, is_enabled()

### Community 7 - "Process Manager"
Cohesion: 0.53
Nodes (5): duration_ms(), capture_output(), files_changed_since(), main(), walk_dir()

### Community 8 - "Verify Script"
Cohesion: 0.70
Nodes (4): check(), check_denied(), send(), verify.sh script

### Community 9 - "Core Design Philosophy"
Cohesion: 0.50
Nodes (4): Core Loop, Design Philosophy, Action Pipeline, BoOS

### Community 10 - "Build & Run Scripts"
Cohesion: 0.67
Nodes (3): Rootfs Build Script, Rust Build Script, QEMU Runner

## Knowledge Gaps
- **22 isolated node(s):** `build-rust.sh script`, `build-rootfs.sh script`, `run-qemu.sh script`, `ParamDef`, `TraceLevel` (+17 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **10 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main()` connect `Process Manager` to `Logging Subsystem`, `Command Registry`, `Gateway & Submit`, `Executor Builtins`?**
  _High betweenness centrality (0.145) - this node is a cross-community bridge._
- **Why does `parse_kv_file()` connect `Command Registry` to `Executor Builtins`, `Capability System`, `Process Manager`?**
  _High betweenness centrality (0.072) - this node is a cross-community bridge._
- **Why does `run_builtin()` connect `Executor Builtins` to `Gateway & Submit`, `Process Manager`?**
  _High betweenness centrality (0.070) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `run_builtin()` (e.g. with `main()` and `main()`) actually correct?**
  _`run_builtin()` has 2 INFERRED edges - model-reasoned connections that need verification._
- **Are the 3 inferred relationships involving `main()` (e.g. with `run_builtin()` and `main()`) actually correct?**
  _`main()` has 3 INFERRED edges - model-reasoned connections that need verification._
- **Are the 4 inferred relationships involving `main()` (e.g. with `Exit code constants (EXIT_ALLOWED/DENIED/ERROR/UNKNOWN)` and `main()`) actually correct?**
  _`main()` has 4 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Read .cmd files and return list of {name, description, capability, exec}.`, `Convert BoOS command registry to OpenAI tool definitions.`, `Write a request file, return (request_id, request_path).` to the rest of the system?**
  _38 weakly-connected nodes found - possible documentation gaps or missing edges._