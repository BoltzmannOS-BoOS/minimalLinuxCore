# Graph Report - .  (2026-05-25)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 122 nodes · 174 edges · 16 communities (12 shown, 4 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 3 edges (avg confidence: 0.82)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `6f083686`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]

## God Nodes (most connected - your core abstractions)
1. `run_builtin()` - 13 edges
2. `run_conversation()` - 11 edges
3. `parse_kv_file()` - 7 edges
4. `log_event()` - 7 edges
5. `boos-exec (execution engine)` - 6 edges
6. `M15 Rust Rewrite (boos multi-call binary)` - 6 edges
7. `write_log_bytes()` - 5 edges
8. `write_request()` - 4 edges
9. `verify.sh script` - 4 edges
10. `generate_id()` - 4 edges

## Surprising Connections (you probably didn't know these)
- `BoOS (README overview)` --semantically_similar_to--> `BoOS (AI-owned computer environment)`  [INFERRED] [semantically similar]
  README.md → PLAN.md
- `DeepSeek V4 Pro Repetition Bug Report` --references--> `M15 Rust Rewrite (boos multi-call binary)`  [INFERRED]
  bug-reports/deepseek-v4-repetition-bug.md → PLAN.md
- `QEMU/initramfs/init boot chain` --conceptually_related_to--> `minimalLinuxCore substrate`  [INFERRED]
  README.md → PLAN.md
- `BoOS Development Conventions (skill)` --references--> `boos-exec (execution engine)`  [EXTRACTED]
  .claude/skills/boos-dev.md → PLAN.md
- `M15 Issues Audit Report` --references--> `boos-submit (request writer)`  [EXTRACTED]
  M15-ISSUES.md → PLAN.md

## Communities (16 total, 4 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.15
Nodes (13): append_log_line(), get_trace_level(), json_escape(), log(), log_allowed(), log_denied(), log_event(), log_unknown() (+5 more)

### Community 1 - "Community 1"
Cohesion: 0.15
Nodes (19): build_command_list_text(), build_tools(), cleanup_old_files(), ensure_dirs(), format_tool_result(), load_command_registry(), main(), make_client() (+11 more)

### Community 2 - "Community 2"
Cohesion: 0.16
Nodes (19): BoOS Development Conventions (skill), DeepSeek V4 Pro Repetition Bug Report, M15 Issues Audit Report, AI Client (ai-client.py), BoOS (AI-owned computer environment), boos-daemon (polling processor), boos-exec (execution engine), boos-gateway (TCP gateway) (+11 more)

### Community 3 - "Community 3"
Cohesion: 0.23
Nodes (13): Command, find_command(), is_enabled(), load_commands(), ParamDef, parse_kv_file(), parse_params(), test_command_backward_compat() (+5 more)

### Community 4 - "Community 4"
Cohesion: 0.26
Nodes (14): check_enabled(), list_commands(), main(), prune_results(), rotate_logs_cmd(), run_builtin(), set_debug(), show_caps() (+6 more)

### Community 5 - "Community 5"
Cohesion: 0.48
Nodes (6): generate_id(), main(), random_suffix(), test_generate_id_format(), test_generate_id_uniqueness_rapid(), test_random_suffix_varies()

### Community 6 - "Community 6"
Cohesion: 0.70
Nodes (4): verify.sh script, check(), check_denied(), send()

### Community 7 - "Community 7"
Cohesion: 0.70
Nodes (4): capture_output(), files_changed_since(), main(), walk_dir()

### Community 8 - "Community 8"
Cohesion: 0.83
Nodes (3): get_auth_token(), handle_connection(), main()

## Knowledge Gaps
- **10 isolated node(s):** `build-rust.sh script`, `run-qemu.sh script`, `Command`, `ParamDef`, `TraceLevel` (+5 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What connects `Read .cmd files and return list of {name, description, capability, exec}.`, `Convert BoOS command registry to OpenAI tool definitions.`, `Write a request file, return (request_id, request_path).` to the rest of the system?**
  _18 weakly-connected nodes found - possible documentation gaps or missing edges._