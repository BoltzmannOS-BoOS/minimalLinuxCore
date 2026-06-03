# Direction A: Filesystem Capabilities — Implementation Plan

> **For Hermes:** Implement task-by-task. TDD where possible. Commit after each task.

**Goal:** Give the BoOS agent write-file, list-dir, and stat commands so it can interact with the filesystem beyond just reading.

**Architecture:** Three new builtins in exec.rs, three new .cmd registry entries, three new capability flags. Same pattern as read-file.

**Tech Stack:** Rust std (std::fs, std::io), no new dependencies.

---

## Task 1: Add capability flags to capabilities.conf

**Objective:** Enable the three new capability flags.

**Files:**
- Modify: `rootfs/etc/boos/capabilities.conf`

**Step 1: Add three new lines**

Append to the file, after `allow_exec=1`:

```
# Filesystem write operations
allow_write_file=1
allow_list_dir=1
allow_stat=1
```

**Step 2: Commit**

```bash
git add rootfs/etc/boos/capabilities.conf
git commit -m "feat: add allow_write_file, allow_list_dir, allow_stat capability flags"
```

---

## Task 2: Create write-file command registry entry

**Objective:** Register write-file as a discoverable command.

**Files:**
- Create: `rootfs/etc/boos/commands/write-file.cmd`

**Step 1: Create the .cmd file**

```
name=write-file
enable_flag=allow_write_file
description=create or overwrite a file
exec=__builtin_write_file
params=path:required,content:required
```

**Step 2: Commit**

```bash
git add rootfs/etc/boos/commands/write-file.cmd
git commit -m "feat: register write-file command"
```

---

## Task 3: Create list-dir command registry entry

**Objective:** Register list-dir.

**Files:**
- Create: `rootfs/etc/boos/commands/list-dir.cmd`

**Step 1: Create the .cmd file**

```
name=list-dir
enable_flag=allow_list_dir
description=list directory contents
exec=__builtin_list_dir
params=path:optional
```

**Step 2: Commit**

```bash
git add rootfs/etc/boos/commands/list-dir.cmd
git commit -m "feat: register list-dir command"
```

---

## Task 4: Create stat command registry entry

**Objective:** Register stat.

**Files:**
- Create: `rootfs/etc/boos/commands/stat.cmd`

**Step 1: Create the .cmd file**

```
name=stat
enable_flag=allow_stat
description=show file metadata (size, type, permissions)
exec=__builtin_stat
params=path:required
```

**Step 2: Commit**

```bash
git add rootfs/etc/boos/commands/stat.cmd
git commit -m "feat: register stat command"
```

---

## Task 5: Implement write-file builtin

**Objective:** Add `__builtin_write_file` handler in exec.rs.

**Files:**
- Modify: `src/rust/src/exec.rs` — add builtin handler
- Modify: `src/rust/src/exec.rs` — add to show_help()

**Step 1: Add the builtin handler in `run_builtin`**

Add this new arm inside the `match exec_target { ... }` block in `fn run_builtin`, after the `__builtin_exec` arm:

```rust
        "__builtin_write_file" => {
            let args_trimmed = args.trim();
            if args_trimmed.is_empty() {
                eprintln!("Usage: write-file <path> <content>");
                return EXIT_ERROR;
            }
            // Split into path + content. Content is everything after the first whitespace-delimited token.
            let space_pos = match args_trimmed.find(|c: char| c.is_whitespace()) {
                Some(p) => p,
                None => {
                    eprintln!("Usage: write-file <path> <content>");
                    return EXIT_ERROR;
                }
            };
            let path = args_trimmed[..space_pos].trim();
            let content = args_trimmed[space_pos..].trim();
            if path.is_empty() || content.is_empty() {
                eprintln!("Usage: write-file <path> <content>");
                return EXIT_ERROR;
            }
            // Create parent directories if needed
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            match std::fs::write(path, content) {
                Ok(()) => {
                    println!("Written: {} ({} bytes)", path, content.len());
                    EXIT_ALLOWED
                }
                Err(e) => {
                    eprintln!("write-file: {}", e);
                    EXIT_ERROR
                }
            }
        }
```

**Step 2: Add to help text in `show_help()`**

Add this line after the `read-file` line:

```rust
    println!("  write-file <path> <content>   create or overwrite a file");
```

**Step 3: Commit**

```bash
git add src/rust/src/exec.rs
git commit -m "feat: implement write-file builtin"
```

---

## Task 6: Implement list-dir builtin

**Objective:** Add `__builtin_list_dir` handler.

**Files:**
- Modify: `src/rust/src/exec.rs` — add builtin handler

**Step 1: Add the builtin handler in `run_builtin`**

After `__builtin_write_file`:

```rust
        "__builtin_list_dir" => {
            let path = args.trim();
            let dir_path = if path.is_empty() { "." } else { path };
            match std::fs::read_dir(dir_path) {
                Ok(entries) => {
                    let mut list: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .collect();
                    list.sort_by_key(|e| e.file_name());
                    println!("Directory: {}", dir_path);
                    for entry in &list {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let file_type = match entry.file_type() {
                            Ok(ft) if ft.is_dir() => "d",
                            Ok(ft) if ft.is_symlink() => "l",
                            Ok(_) => "-",
                            Err(_) => "?",
                        };
                        let size = entry.metadata()
                            .map(|m| m.len())
                            .unwrap_or(0);
                        println!("  {} {:>8} {}", file_type, size, name);
                    }
                    println!("  ({} entries)", list.len());
                    EXIT_ALLOWED
                }
                Err(e) => {
                    eprintln!("list-dir: {}", e);
                    EXIT_ERROR
                }
            }
        }
```

**Step 2: Add to help text**

After `write-file` line:

```rust
    println!("  list-dir [path]               list directory contents");
```

**Step 3: Commit**

```bash
git add src/rust/src/exec.rs
git commit -m "feat: implement list-dir builtin"
```

---

## Task 7: Implement stat builtin

**Objective:** Add `__builtin_stat` handler.

**Files:**
- Modify: `src/rust/src/exec.rs` — add builtin handler

**Step 1: Add the builtin handler**

After `__builtin_list_dir`:

```rust
        "__builtin_stat" => {
            let path = args.trim();
            if path.is_empty() {
                eprintln!("Usage: stat <path>");
                return EXIT_ERROR;
            }
            let p = std::path::Path::new(path);
            match std::fs::metadata(p) {
                Ok(m) => {
                    let ftype = if m.is_dir() { "directory" }
                        else if m.is_symlink() { "symlink" }
                        else if m.is_file() { "file" }
                        else { "other" };
                    println!("File: {}", path);
                    println!("  Type: {}", ftype);
                    println!("  Size: {} bytes", m.len());
                    if let Ok(perm) = std::fs::metadata(p) {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = perm.permissions().mode();
                        println!("  Permissions: {:o}", mode & 0o777);
                    }
                    if let Ok(mtime) = m.modified() {
                        if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                            println!("  Modified: {} (epoch)", dur.as_secs());
                        }
                    }
                    if let Ok(created) = m.created() {
                        if let Ok(dur) = created.duration_since(std::time::UNIX_EPOCH) {
                            println!("  Created: {} (epoch)", dur.as_secs());
                        }
                    }
                    EXIT_ALLOWED
                }
                Err(e) => {
                    eprintln!("stat: {}", e);
                    EXIT_ERROR
                }
            }
        }
```

**Step 2: Add to help text**

After `list-dir` line:

```rust
    println!("  stat <path>                   show file metadata");
```

**Step 3: Commit**

```bash
git add src/rust/src/exec.rs
git commit -m "feat: implement stat builtin"
```

---

## Task 8: Update help builtin text (covered inline in Tasks 5-7)

Already done — each task adds its line to `show_help()`. No separate commit needed.

---

## Task 9: Build and run unit tests

**Objective:** Verify Rust code compiles and existing tests still pass.

**Step 1: Build**

```bash
cd src/rust
cargo build --release --target x86_64-unknown-linux-musl
```

Expected: BUILD SUCCESS, no new warnings.

**Step 2: Run unit tests**

```bash
cargo test
```

Expected: All existing tests pass (20+ tests).

**Step 3: Commit if any fixes needed**

Only if build fails — fix, then commit.

---

## Task 10: Add integration tests for new commands

**Objective:** Add test cases for write-file, list-dir, stat in the integration test suite.

**Files:**
- Modify: `tests/integration-test.sh`

**Step 1: Add test section after the existing read-file test**

Find the `read-file` test section and add after it:

```bash
# --- 8. File write operations ---
echo ""
echo "--- 8. File Write Operations ---"

# 8.1 write-file creates a file
out=$(send "write-file /tmp/boos-test.txt hello world")
if echo "$out" | grep -q "Written"; then
    echo "  PASS: write-file creates file"
    PASS=$((PASS + 1))
else
    echo "  FAIL: write-file does not report success"
    echo "    got: $out"
    FAIL=$((FAIL + 1))
fi

# 8.2 read-file confirms the write
out=$(send "read-file /tmp/boos-test.txt")
if echo "$out" | grep -q "hello world"; then
    echo "  PASS: read-file confirms write-file"
    PASS=$((PASS + 1))
else
    echo "  FAIL: read-file cannot confirm write"
    echo "    got: $out"
    FAIL=$((FAIL + 1))
fi

# --- 9. Directory listing ---
echo ""
echo "--- 9. Directory Listing ---"

# 9.1 list-dir shows /tmp
out=$(send "list-dir /tmp")
if echo "$out" | grep -q "boos-test.txt"; then
    echo "  PASS: list-dir shows created file"
    PASS=$((PASS + 1))
else
    echo "  FAIL: list-dir does not show test file"
    echo "    got: $out"
    FAIL=$((FAIL + 1))
fi

# 9.2 list-dir defaults to current dir
out=$(send "list-dir")
if echo "$out" | grep -q "entries"; then
    echo "  PASS: list-dir works without path"
    PASS=$((PASS + 1))
else
    echo "  FAIL: list-dir without path fails"
    echo "    got: $out"
    FAIL=$((FAIL + 1))
fi

# --- 10. File stat ---
echo ""
echo "--- 10. File Stat ---"

# 10.1 stat on created file
out=$(send "stat /tmp/boos-test.txt")
if echo "$out" | grep -q "Type: file"; then
    echo "  PASS: stat identifies file type"
    PASS=$((PASS + 1))
else
    echo "  FAIL: stat does not identify file type"
    echo "    got: $out"
    FAIL=$((FAIL + 1))
fi

# 10.2 stat on directory
out=$(send "stat /tmp")
if echo "$out" | grep -q "Type: directory"; then
    echo "  PASS: stat identifies directory type"
    PASS=$((PASS + 1))
else
    echo "  FAIL: stat does not identify directory"
    echo "    got: $out"
    FAIL=$((FAIL + 1))
fi

# 10.3 stat on nonexistent path
out=$(send "stat /nonexistent")
if echo "$out" | grep -q "stat:"; then
    echo "  PASS: stat errors on nonexistent path"
    PASS=$((PASS + 1))
else
    echo "  FAIL: stat does not error on nonexistent"
    echo "    got: $out"
    FAIL=$((FAIL + 1))
fi
```

**Step 2: Update final summary count**

Find the `echo "Total:"` line and change the expected total to account for +7 new tests.

**Step 3: Commit**

```bash
git add tests/integration-test.sh
git commit -m "test: add integration tests for write-file, list-dir, stat"
```

---

## Task 11: Run full build + integration test

**Objective:** Build everything and verify end-to-end in QEMU or Docker.

**Step 1: Build full initramfs**

```bash
bash scripts/build-rust.sh
bash scripts/build-rootfs.sh
```

**Step 2: Boot in QEMU and run integration tests**

```bash
bash tests/integration-test.sh localhost 5555
```

Expected: 26/26 tests pass (19 existing + 7 new).

**Step 3: If tests fail, fix and recommit**

---

## Verification Checklist

- [ ] `capabilities.conf` has `allow_write_file`, `allow_list_dir`, `allow_stat` flags
- [ ] Three new `.cmd` files exist in `commands/`
- [ ] `write-file /tmp/test.txt some content` creates file
- [ ] `read-file /tmp/test.txt` returns "some content"
- [ ] `list-dir /tmp` shows test.txt with correct size
- [ ] `stat /tmp/test.txt` shows Type: file with correct size
- [ ] `stat /tmp` shows Type: directory
- [ ] `stat /nonexistent` returns error
- [ ] All integration tests pass
- [ ] `cargo test` passes
- [ ] Agent can discover new commands via `commands`
