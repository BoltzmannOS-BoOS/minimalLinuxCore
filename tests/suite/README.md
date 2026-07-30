Tests run: Thu  4 Jun 2026 02:44:33 UTC

## Test Suite Summary

| Category | Files | Status |
|----------|-------|--------|
| archived unit snapshot | cargo-test-output.txt | 41/41 PASS |
| current unit suite | `cargo test` | 139/139 PASS on 2026-07-30 |
| integration/ | integration-test.sh | 24 checks defined |
| demo/    | run-demo.sh | PASS |
| e2e/     | direction-c-audit-demo.log | Ran |
| research/ | semantic-object-view/ | Protocol + validator |

## Reproducibility

All commits since project start:

\`\`\`
$(cat tests/suite/git-log.txt)
\`\`\`

Current HEAD: `$(cat tests/suite/git-head.txt)`

To reproduce this exact state:
```bash
git checkout $(cat tests/suite/git-head.txt)
cd src/rust && cargo test   # 41/41 expected for the archived commit
bash tests/suite/integration/integration-test.sh localhost 5555  # if BoOS gateway is running
```

For the current candidate, run `cargo test` from `src/rust`; the gateway
integration suite additionally requires a running BoOS instance.

## Directions Implemented

| Direction | Commands | Tests |
|-----------|----------|-------|
| A — Filesystem | write-file, list-dir, stat | 10 integration tests |
| B — Develop Agent | READ/WRITE/BUILD/TEST/DONE loop | 15 unit tests |
| C — Audit | audit recent/failures/session/summary | CLI smoke test |
