# M15 Issues — 审计报告

本次审计日期：2026-05-25
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

### B7. `generate_id` 确定性碰撞

**文件：** `src/rust/src/submit.rs:12-21`

**问题：** 后缀 `(uptime * 10000) % 10000` 在快速连续调用中不变。O_EXCL 重试提供兜底，但 ID 质量弱。

**建议：** 使用 `/dev/urandom` 的 4 字节替代确定性后缀。

### B8. exit code 语义模糊

**文件：** `src/rust/src/process.rs:220-226`

**问题：** exit code 1 既代表 capability 拒绝也代表命令执行失败，导致未知命令被误标为 "denied"。

**建议：** 区分退出码：0=成功，1=拒绝，2=错误，3=未知命令。

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

### A1. Gateway 单线程瓶颈

**证据：** 200 并发连接中约 64 个失败（连接被拒绝或超时）。Gateway 使用单线程 `listener.incoming()` 循环，`handle_connection` 阻塞。

**影响：** AI 并发工具调用时可能遇到超时。

**建议：** 至少用 `thread::spawn` 处理每个连接，或使用连接池。

### A2. Gateway panic 风险

**文件：** `src/rust/src/gateway.rs:23-26`

```rust
let mut reader = BufReader::new(stream.try_clone().unwrap_or_else(|| {
    panic!("cannot clone stream");
}));
```

单个连接克隆失败会导致整个 gateway 崩溃。

### A3. verbose 模式 1 秒阻塞延迟

**文件：** `src/rust/src/process.rs:178`

```rust
std::thread::sleep(std::time::Duration::from_secs(1));
```

每个请求在 verbose 模式下额外延迟 1 秒，大幅降低吞吐量。

### A4. 未使用的代码/死代码

编译器警告暴露：
- 4 个常量未使用（`DAEMON_DIR`, `DAEMON_RUN_DIR`, `MAX_LOG_LINE_LEN`, `QUEUE_POLL_MS`）
- `fmt_ts` 函数未使用
- `Command.params` 和 `ParamDef` 字段已定义但从未被读取——说明 `.cmd` 文件中的 `params` 解析已实现但未在实际工具生成中使用

### A5. 持久化 /var 保留旧数据

跨启动保留的持久化 /var 会累积旧的日志、result 文件，导致 `results` 和 `log` 命令输出膨胀。没有日志轮转或清理机制。

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

### Rust 单元测试（新增）
**22 个测试全部通过** — 覆盖 KV 解析、JSON 转义、ID 生成、参数解析、时长计算

### verify.sh 集成测试
**26/27 通过** — 1 个超时相关失败

### 静态分析（自定义 Python 工具）
生产脚本发现 **118 个问题**（28 HIGH / 79 MED / 11 LOW），主要集中在残留 shell 脚本中

---

## 六、优先修复状态

**已在本次审计中修复 (6 个):**
- ✅ B1 日志写入交叠
- ✅ B2 verbose fs 追踪（epoch vs uptime 错配）
- ✅ B3 管道死锁（stdout/stderr 顺序读）
- ✅ B4 Gateway panic（try_clone 崩溃）
- ✅ B5 Gateway 读超时（无声客户端 DoS）
- ✅ B6 重试循环（tmp_path 残留导致重试无效）

**仍待修复 (2 个):**
- B7 ID 生成加真正的随机数
- B8 exit code 语义区分（denied vs error）

**仍然在 shell 中的 (6 个):**
- S1-S6 boos-shell + boos-supervisor + boos-daemon
