# M15 Issues — 审计报告

本次审计日期：2026-05-25
第二轮修复：2026-05-25（Claude Opus 4.7）
审计范围：Rust 重写（boos-exec, boos-process, boos-submit, boos-gateway）+
残留 shell 脚本（boos-shell, boos-supervisor, boos-daemon, init）

---

## 一、已确认的 Bug（Rust 代码）

### B1. 日志写入交叠 — ✅ 已修复

**文件：** `src/rust/src/log.rs:136-141`

**问题：** `append_log_line` 将内容和换行符分两次 `write_all` 调用。多进程并发写同一日志文件时导致行交叠。

**修复：** `\n` 合并到同一 buffer 中，单次 `write_all`。

### B2. verbose fs 追踪完全失效 — ✅ 已修复

**文件：** `src/rust/src/process.rs:180`

**问题：** `marker_ts` 使用 `uptime_secs()`（~100s），但 `files_changed_since` 比较文件 mtime（~17.7 亿秒 UNIX epoch）。所有文件被误报为已修改。

**修复：** 改用 `SystemTime::now().duration_since(UNIX_EPOCH)`。

### B3. 管道死锁 — ✅ 已修复

**文件：** `src/rust/src/process.rs:32-50` — **严重**

**问题：** 先读 stdout 再读 stderr。如果子进程先产生大量 stderr（>64KB 管道缓冲），子进程阻塞在 stderr 写入，stdout 永不关闭，`read_to_end` 永不返回 → 死锁。

**证据：** Python 模拟测试在 5 秒内触发超时死锁。

**修复：** 改为两个线程并发读取 stdout 和 stderr。

### B4. Gateway `try_clone()` panic — ✅ 已修复

**文件：** `src/rust/src/gateway.rs:23-26`

**问题：** `stream.try_clone()` 失败时直接 `panic!`，整个 gateway 进程崩溃。

**修复：** 改为记录错误日志并 `return` 断开连接。

### B5. Gateway 无读超时 — ✅ 已修复

**文件：** `src/rust/src/gateway.rs:30` — **DoS 漏洞**

**问题：** 客户端连接后不发数据，`read_line` 永久阻塞。单线程 gateway 对所有后续连接不可用。

**修复：** 添加 `stream.set_read_timeout(Some(Duration::from_secs(30)))`。

### B6. 重试循环脆弱 — ✅ 已修复

**文件：** `src/rust/src/submit.rs:55-78`

**问题：** 初次尝试失败后，所有重试都用同一个 `tmp_path`。如果 `tmp_path` 也碰巧存在（前次崩溃残留），所有 10 次重试都命中同一个已存在的文件 → 全部失败。

**修复：** 重试时先 `remove_file(&tmp_path)` 清理残留，加 1ms 退避。

### B7. `generate_id` 确定性碰撞 — ✅ 已修复

**文件：** `src/rust/src/submit.rs:11-33`

**问题：** 后缀 `(uptime * 10000) % 10000` 在快速连续调用中不变。O_EXCL 重试提供兜底，但 ID 质量弱。

**修复：** 从 `/dev/urandom` 读 4 字节作 8 位十六进制后缀。`random_suffix()` 失败时回退到 `(uptime ^ pid)` 混合。新增 `test_generate_id_uniqueness_rapid` 测试 1000 次连续生成无碰撞。

### B8. exit code 语义模糊 — ✅ 已修复

**文件：** `src/rust/src/config.rs`, `src/rust/src/exec.rs`, `src/rust/src/process.rs`

**问题：** exit code 1 既代表 capability 拒绝也代表命令执行失败，导致未知命令被误标为 "denied"。

**修复：** 在 `config.rs` 定义退出码契约：`EXIT_ALLOWED=0`, `EXIT_DENIED=1`, `EXIT_ERROR=2`, `EXIT_UNKNOWN=3`。`exec.rs` 三个分支分别走各自的退出码；`process.rs` verdict 映射新增 "unknown" 分类。外部程序通过 `exec=` 调用时退出码透传，process.rs 将 {0,1,3} 之外的值映射为 "error"。

---

## 二、残留 Shell 脚本问题

以下组件尚未用 Rust 重写，仍存在已知问题：

### S1. boos-shell — glob 注入

**文件：** `rootfs/bin/boos-shell:24`

```sh
set -- $line  # 无 set -f，glob 字符展开
```

### S2. boos-shell — read 无 -r

**文件：** `rootfs/bin/boos-shell:18`

```sh
read line  # 无 -r，反斜杠被解释
```

### S3. boos-supervisor — 未加引号的 daemon 启动

**文件：** `rootfs/bin/boos-supervisor:65`

```sh
$exec_cmd &  # glob 展开 + 单词拆分
```

### S4. boos-supervisor — PID 文件 TOCTOU

**文件：** `rootfs/bin/boos-supervisor:66-68`

### S5. boos-daemon — 错误静默丢弃

**文件：** `rootfs/bin/boos-daemon:15,18`

所有错误重定向到 `/dev/null`。

### S6. boos-daemon — 1 秒轮询

Rust `config.rs` 定义了 `QUEUE_POLL_MS = 200`，但实际使用的 daemon 仍是 shell 脚本，硬编码 `sleep 1`。

---

## 三、架构层面问题

### A1. Gateway 单线程瓶颈 — ✅ 已修复

**证据：** 200 并发连接中约 64 个失败（连接被拒绝或超时）。Gateway 使用单线程 `listener.incoming()` 循环，`handle_connection` 阻塞。

**修复：** `gateway.rs` 主循环改为 `thread::spawn` per connection；用 `Arc<AtomicUsize>` 计数器限制并发到 `MAX_GATEWAY_THREADS = 64`，超额连接收到 `BUSY\n` 后断开。每个线程结束时递减计数。

### A2. Gateway panic 风险 — ✅ 已修复（B4 同步解决）

**文件：** `src/rust/src/gateway.rs`

B4 修复时已将 `try_clone().unwrap_or_else(|| panic!)` 改为 match + 日志 + return。本条与 B4 实为同一处代码。

### A3. verbose 模式 1 秒阻塞延迟 — ✅ 已修复

**文件：** `src/rust/src/process.rs`

**修复：** 删除 marker 文件创建逻辑（marker 从未真正参与比较）和 1 秒 sleep。改为 `SystemTime::now() - 1s` 直接作为查找基准，ext2 整秒粒度的边界情况通过 1s 回看窗口覆盖。代价：至多 1 秒前的预存改动会作为 false positive 出现在 fs_trace，可接受。

### A4. 未使用的代码 / 死代码 — ✅ 已修复

- 删除 `fmt_ts` 函数及其 2 个测试
- 删除 `DAEMON_DIR`, `DAEMON_RUN_DIR`, `QUEUE_POLL_MS` 常量（boos-supervisor 仍是 shell，无 Rust 引用方）
- `MAX_LOG_LINE_LEN`：`log::write_log_bytes` 中现已实际截断超长行
- `Command.params` / `ParamDef`：新增 `commands --json` 模式输出结构化 JSON（含 params），AI 客户端可消费此格式生成带参数的 tool definition。同时给 `submit.cmd`, `result.cmd`, `debug.cmd`, `prune.cmd` 补上 `params=` 字段

### A5. 持久化 /var 保留旧数据 — ✅ 已修复

- **日志轮转：** `log::write_log_bytes` 每 64 次写入检查一次文件大小，超过 `MAX_LOG_BYTES = 10MB` 时 `boos.log → boos.log.1 → boos.log.2`，保留 2 份历史
- **手动结果清理：** 新增 `prune [days]` builtin，删除 `/var/boos/results/*.out` 中 mtime 早于 N 天的文件（默认 7 天）
- **手动日志轮转：** 新增 `rotate-logs` builtin 强制立即轮转

保留"observe, don't obstruct"原则：prune 是手动触发，不自动；轮转后旧日志保留为 .1/.2 而非删除。

---

## 四、已修复（与 shell 版本对比）

以下 shell 版本的问题已在 Rust 重写中解决：

| Shell 问题 | Rust 修复方式 |
|-----------|-------------|
| glob 展开注入 | Rust `process::Command` 不使用 shell |
| 字符串匹配判断 verdict | 基于 exit code（仍不完美，见 B4） |
| 输出无大小限制 | `MAX_OUTPUT_BYTES = 1MB` 硬上限 + 截断 |
| 文件写入非原子 | 所有写入使用 `tmp + rename` 模式 |
| requester 可伪造（`-r` 参数） | 仅从环境变量 `BOOS_REQUESTER` 读取，不接受 CLI 覆盖 |
| Gateway 无认证 | 可选的 `BOOS_GATEWAY_TOKEN` / `/etc/boos/gateway_token` |
| 日志格式（纯文本 key=value） | 改为 JSON 行 |
| 请求 ID 碰撞 | `SystemTime::as_millis()` + O_EXCL 重试（改善但非完美，见 B3） |
| `nc -ll` 不可靠 | 改用 Rust `TcpListener` |
| AI 工具定义硬编码参数 | `registry.rs` 支持从 `.cmd` 解析 `params` 字段（但尚未接入客户端） |

---

## 五、测试结果

### Rust 单元测试
**第一轮：** 22 个测试通过（DeepSeek 实现）
**第二轮（本次）：** 20 个测试通过
- 删除 `fmt_ts` 的 2 个测试（函数被删除）
- 替换 2 个 "documented-collision" 测试为真正的唯一性测试（B7 后无碰撞）
- 新增 `test_random_suffix_varies`

### verify.sh 集成测试
**第一轮：** 26/27 通过（在 DeepSeek 的测试环境）
**第二轮：** 在当前开发机环境下无法执行 — `build/vmlinuz` 是 Ubuntu generic 内核，没有把 virtio-net 编译进 vmlinuz（要从模块加载），导致 QEMU 用户态网络的 host→guest 端口转发拿不到响应。这是宿主机内核选择问题，**不是 Rust 代码问题**。需要由用户在原先 verify.sh 通过的内核环境下重跑。

### 静态分析（自定义 Python 工具）
生产脚本发现 **118 个问题**（28 HIGH / 79 MED / 11 LOW），主要集中在残留 shell 脚本中

---

## 六、优先修复状态

**第一轮 DeepSeek 已修复 (6 个):**
- ✅ B1 日志写入交叠
- ✅ B2 verbose fs 追踪（epoch vs uptime 错配）
- ✅ B3 管道死锁（stdout/stderr 顺序读）
- ✅ B4 Gateway panic（try_clone 崩溃）
- ✅ B5 Gateway 读超时（无声客户端 DoS）
- ✅ B6 重试循环（tmp_path 残留导致重试无效）

**第二轮 Claude 修复 (7 项):**
- ✅ B7 `/dev/urandom` 随机后缀
- ✅ B8 退出码语义契约（allowed/denied/error/unknown）
- ✅ A1 Gateway 多线程 + 并发上限
- ✅ A2 与 B4 同一处，已解决
- ✅ A3 删除 verbose 模式的 1 秒 sleep
- ✅ A4 删除死代码 + `commands --json` 接出 params + MAX_LOG_LINE_LEN 实际生效
- ✅ A5 日志轮转 + `prune` / `rotate-logs` builtin

**仍然在 shell 中的 (6 个):**
- S1-S6 boos-shell + boos-supervisor + boos-daemon

下一步候选：将 boos-supervisor 也 Rust 化（替换 shell 中的 awk /proc/uptime 调用、PID TOCTOU），或为残余 shell 加 `set -fu` + 引号化全部展开。
