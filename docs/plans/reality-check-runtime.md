# BoOS Reality Check Runtime — Implementation Plan

## Goal
Prevent "vocabulary mismatch → false negative" when researching whether a concept exists in the real world.

## Architecture
```
Hermes CLI
  ↓ MCP stdio protocol
boos-research-mcp (Python, ~300 lines)
  ↓
Search Providers
  ├── web_search tool (existing Hermes SearXNG) — free
  ├── arXiv API (free, no key needed)
  ├── GitHub code search (free, rate-limited)
  └── local SQLite trace DB
```

## Phase 1: MCP Server Skeleton (today)
- [ ] Create `tools/boos-research-mcp/server.py`
- [ ] Stdio MCP server with two tools
- [ ] Register with `hermes mcp add boos-research --command python3 server.py`
- [ ] Verify tool discovery

## Phase 2: research_concept tool (today)
- [ ] Input: concept string, claim to test
- [ ] Step 1: Concept normalization → 3-5 naming spaces
- [ ] Step 2: Multi-source search (web + arXiv + GitHub)
- [ ] Step 3: Deduplicate and rank results
- [ ] Step 4: Return with confidence level

## Phase 3: search_trace tool (today)
- [ ] Store all queries, results, confidence in SQLite
- [ ] Return query expansion tree + sources + near-misses

## Phase 4: Test & Validate (today)
- [ ] Search "agent self-state" → expect Springdrift in results
- [ ] Search "agent version control" → expect AgentGit in results
- [ ] Compare vs single-keyword search → prove fewer false negatives

## Files to create
```
tools/boos-research-mcp/
  server.py           — MCP stdio server (~200 lines)
  research.py         — concept expansion + search logic (~100 lines)
  trace.py            — SQLite trace storage (~50 lines)
  requirements.txt    — mcp (only dependency)
```

## Hermes registration
```bash
hermes mcp add boos-research --command "python3 tools/boos-research-mcp/server.py"
```

## Success criteria
- research_concept("agent runtime self-state") returns Springdrift in top 5
- search_trace shows query expansion from "self-state" → "sensorium" → "ambient self-perception"
- Compare: web_search("agent self-state") alone does NOT return Springdrift
