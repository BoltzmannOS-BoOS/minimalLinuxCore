//! Autonomous exploration engine for BoOS.
//!
//! `boos-agent explore` runs without external LLM — it uses a built-in
//! curiosity list to discover what BoOS can and cannot do, recording
//! everything via the memory system.
//!
//! Architecture:
//!   Phase 1: Try all registered commands → record capabilities
//!   Phase 2: Try curiosity commands → find gaps
//!   Phase 3: Generate exploration report

use std::fs;
use std::process::Command;

use crate::log;
use crate::memory;
use crate::registry;

// ── Curiosity entry ────────────────────────────────────────────────────────

struct Curiosity {
    command: &'static str,
    args: &'static str,
    category: &'static str,
    reason: &'static str,
}

/// Built-in knowledge base: "In any AI-capable OS, these should exist."
/// Each entry comes from:
///   - Linux/Unix tradition (ls, cat, ps, etc.)
///   - AI frameworks (embed, search, classify)
///   - DevOps ecosystems (deploy, sandbox, checkpoint)
///   - Science fiction (clone, dream, snapshot)
///   - Agent systems (delegate, task, review)
const CURIOSITY_LIST: &[Curiosity] = &[
    // ── Unix essentials ──
    Curiosity { command: "ls", args: "", category: "filesystem", reason: "任何OS都能列出文件" },
    Curiosity { command: "cat", args: "/etc/hostname", category: "filesystem", reason: "读文件是最基础的操作" },
    Curiosity { command: "ps", args: "", category: "process", reason: "进程列表是系统管理的基础" },
    Curiosity { command: "whoami", args: "", category: "identity", reason: "AI应该知道自己是谁" },
    Curiosity { command: "dmesg", args: "", category: "system", reason: "内核日志帮助理解启动过程" },
    Curiosity { command: "mount", args: "", category: "filesystem", reason: "了解文件系统布局" },

    // ── File operations ──
    Curiosity { command: "read-file", args: "/etc/hostname", category: "filesystem", reason: "语义化文件读取" },
    Curiosity { command: "read", args: "/etc/hostname", category: "filesystem", reason: "更简短的文件读取" },
    Curiosity { command: "write-file", args: "/tmp/test", category: "filesystem", reason: "AI需要写出产物" },
    Curiosity { command: "write", args: "/tmp/test", category: "filesystem", reason: "简写版本" },
    Curiosity { command: "edit", args: "/tmp/test", category: "filesystem", reason: "编辑已有文件" },
    Curiosity { command: "append", args: "/tmp/test line", category: "filesystem", reason: "追加内容而不覆盖" },
    Curiosity { command: "mkdir", args: "/tmp/newdir", category: "filesystem", reason: "创建目录" },
    Curiosity { command: "touch", args: "/tmp/newfile", category: "filesystem", reason: "Unix标准创建文件方式" },
    Curiosity { command: "rm", args: "/tmp/test", category: "filesystem", reason: "清理能力" },
    Curiosity { command: "list-dir", args: "/", category: "filesystem", reason: "语义化目录列表" },

    // ── Network ──
    Curiosity { command: "curl", args: "http://localhost", category: "network", reason: "HTTP客户端是互联网的基础" },
    Curiosity { command: "wget", args: "http://localhost", category: "network", reason: "文件下载" },
    Curiosity { command: "ping", args: "127.0.0.1", category: "network", reason: "网络连通性检查" },
    Curiosity { command: "fetch", args: "http://localhost", category: "network", reason: "通用获取接口" },
    Curiosity { command: "connect", args: "localhost:8080", category: "network", reason: "TCP直连" },

    // ── AI native ──
    Curiosity { command: "learn", args: "sample", category: "ai", reason: "从数据中学习是AI的核心能力" },
    Curiosity { command: "predict", args: "test", category: "ai", reason: "推理能力" },
    Curiosity { command: "embed", args: "hello world", category: "ai", reason: "文本向量化是RAG的基础" },
    Curiosity { command: "search", args: "keyword", category: "ai", reason: "语义搜索区别于文本匹配" },
    Curiosity { command: "classify", args: "text", category: "ai", reason: "分类是基础ML任务" },
    Curiosity { command: "generate", args: "prompt", category: "ai", reason: "文本生成" },
    Curiosity { command: "plan", args: "goal", category: "ai", reason: "任务规划是agent的核心" },
    Curiosity { command: "reason", args: "question", category: "ai", reason: "多步推理" },

    // ── Knowledge management ──
    Curiosity { command: "graph", args: "", category: "knowledge", reason: "知识图谱操作" },
    Curiosity { command: "query", args: "SELECT", category: "knowledge", reason: "结构化查询" },
    Curiosity { command: "kb", args: "", category: "knowledge", reason: "知识库管理" },
    Curiosity { command: "vector", args: "text", category: "knowledge", reason: "向量操作" },
    Curiosity { command: "similarity", args: "a b", category: "knowledge", reason: "相似度计算" },

    // ── Sci-fi / advanced ──
    Curiosity { command: "clone", args: "", category: "scifi", reason: "自我复制——任何agent系统的自然需求" },
    Curiosity { command: "fork", args: "", category: "scifi", reason: "Unix fork的AI版本" },
    Curiosity { command: "spawn", args: "worker", category: "scifi", reason: "创建子agent" },
    Curiosity { command: "checkpoint", args: "before-risky-op", category: "scifi", reason: "状态快照——操作前的安全网" },
    Curiosity { command: "snapshot", args: "", category: "scifi", reason: "全系统快照——ZFS概念迁移" },
    Curiosity { command: "rollback", args: "before-risky-op", category: "scifi", reason: "回滚——git revert的OS版本" },
    Curiosity { command: "restore", args: "snapshot-1", category: "scifi", reason: "恢复状态" },
    Curiosity { command: "dream", args: "scenario", category: "scifi", reason: "离线想象——AlphaGo self-play概念" },
    Curiosity { command: "imagine", args: "scenario", category: "scifi", reason: "假设推理" },
    Curiosity { command: "simulate", args: "scenario", category: "scifi", reason: "模拟场景" },
    Curiosity { command: "reflect", args: "", category: "scifi", reason: "自我反思——分析自己的记忆" },
    Curiosity { command: "merge-memory", args: "session-x", category: "scifi", reason: "融合其他session的知识" },

    // ── Agent collaboration ──
    Curiosity { command: "delegate", args: "task", category: "agent", reason: "委派任务——多Agent协作的基础" },
    Curiosity { command: "task", args: "list", category: "agent", reason: "任务管理" },
    Curiosity { command: "review", args: "code", category: "agent", reason: "代码审查" },
    Curiosity { command: "approve", args: "change", category: "agent", reason: "审批流程" },
    Curiosity { command: "broadcast", args: "message", category: "agent", reason: "广播消息给所有agent" },
    Curiosity { command: "whisper", args: "agent-2 msg", category: "agent", reason: "点对点通信" },
    Curiosity { command: "consensus", args: "", category: "agent", reason: "共识算法——多agent决策" },

    // ── Security and isolation ──
    Curiosity { command: "sandbox", args: "untrusted-cmd", category: "security", reason: "沙箱执行——不可信代码的基础需求" },
    Curiosity { command: "jail", args: "process", category: "security", reason: "FreeBSD jail概念" },
    Curiosity { command: "seal", args: "data", category: "security", reason: "密封存储——SGX概念" },
    Curiosity { command: "audit", args: "", category: "security", reason: "安全审计" },
    Curiosity { command: "attest", args: "", category: "security", reason: "远程认证" },
    Curiosity { command: "verify", args: "hash", category: "security", reason: "完整性验证" },

    // ── Monitoring ──
    Curiosity { command: "watch", args: "status", category: "monitor", reason: "持续观察——inotify的AI接口" },
    Curiosity { command: "monitor", args: "cpu", category: "monitor", reason: "指标监控" },
    Curiosity { command: "probe", args: "health", category: "monitor", reason: "健康检查" },
    Curiosity { command: "heartbeat", args: "", category: "monitor", reason: "心跳信号" },
    Curiosity { command: "pulse", args: "", category: "monitor", reason: "脉冲检测" },

    // ── Config management ──
    Curiosity { command: "config", args: "", category: "config", reason: "配置管理" },
    Curiosity { command: "set-config", args: "key val", category: "config", reason: "运行时修改配置" },
    Curiosity { command: "get-config", args: "key", category: "config", reason: "读取配置" },
    Curiosity { command: "env", args: "", category: "config", reason: "环境变量" },
    Curiosity { command: "secret", args: "list", category: "config", reason: "密钥管理" },
    Curiosity { command: "enable", args: "shell", category: "config", reason: "动态启用能力" },
    Curiosity { command: "disable", args: "shell", category: "config", reason: "动态禁用能力" },
];

// ── Exploration engine ─────────────────────────────────────────────────────

struct ExploreResult {
    total_tried: u32,
    succeeded: u32,
    failed: u32,
    gaps: Vec<(String, String, String)>,  // (command, category, reason)
    discoveries: Vec<(String, String)>,    // (command, output_summary)
}

fn try_command(_known: bool, cmd: &str, args: &str) -> Option<String> {
    let _full_cmd = if args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, args)
    };

    // Use boos-exec directly (same binary, different argv[0])
    let output = match Command::new("/bin/boos-exec")
        .arg(cmd)
        .args(args.split_whitespace().collect::<Vec<_>>())
        .output()
    {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            let combined = format!("{}{}", stdout, stderr).trim().to_string();
            if combined.is_empty() {
                format!("(exit={})", o.status.code().unwrap_or(-1))
            } else {
                combined
            }
        }
        Err(e) => format!("(spawn error: {})", e),
    };

    if output.contains("Unknown command")
        || output.contains("Permission denied")
    {
        None
    } else {
        Some(output)
    }
}

fn observe_and_remember(session_id: &str, fact: &str, key: &str, value: &str, tags: &str) {
    // Record observation
    let entry = memory::RecentEntry::new("observation", fact, session_id);
    let _ = memory::recent_add(entry);

    // Add to working memory
    if let Ok(mut wm) = memory::WorkingMemory::load() {
        wm.add_fact(fact);
        let _ = wm.save();
    }

    // Persist to archive
    let _ = memory::archive_set(key, value, session_id, tags);
}

/// Run autonomous exploration without external LLM.
pub fn run_explore(bold: bool) {
    let total_start = memory::now_secs();
    let session_id = format!("auto-explore-{}", total_start);

    // Start session
    let _ = memory::session_start(&session_id);
    log::log("boos-explore", "started", &[
        ("session", &log::json_escape(&session_id)),
        ("bold", if bold { "true" } else { "false" }),
    ]);

    let mut result = ExploreResult {
        total_tried: 0,
        succeeded: 0,
        failed: 0,
        gaps: Vec::new(),
        discoveries: Vec::new(),
    };

    // ── Phase 1: Try all registered commands ──
    println!("╔══════════════════════════════════════╗");
    println!("║  BoOS Autonomous Exploration        ║");
    println!("║  Session: {:<24} ║", session_id);
    println!("╚══════════════════════════════════════╝");
    println!();

    println!("── Phase 1: 尝试已注册命令 ──");
    let commands = registry::load_commands();
    for cmd in &commands {
        print!("  {} ... ", cmd.name);
        result.total_tried += 1;

        let args = if cmd.params.is_empty() {
            ""
        } else {
            // Provide a dummy arg for required params
            let required: Vec<_> = cmd.params.iter()
                .filter(|p| p.required)
                .collect();
            if required.is_empty() { "" } else { "test" }
        };

        match try_command(true, &cmd.name, args) {
            Some(output) => {
                result.succeeded += 1;
                let summary: String = output.lines().take(1).collect();
                let summary = if summary.len() > 80 {
                    format!("{}...", &summary[..77])
                } else {
                    summary
                };
                println!("✓ {}", summary);

                let fact = format!("发现: {} — {}", cmd.name, cmd.description);
                let key = format!("explore_cmd_{}", cmd.name.replace('-', "_"));
                observe_and_remember(&session_id, &fact, &key, &cmd.description, "discovery,capability");
                result.discoveries.push((cmd.name.clone(), summary));
            }
            None => {
                result.failed += 1;
                println!("✗ (returned error)");
            }
        }
    }

    println!();
    println!("  注册命令: {} 成功, {} 失败 / {} 总数",
        result.succeeded, result.failed, result.total_tried);

    // ── Phase 2: Try curiosity commands (if bold mode) ──
    if bold {
        println!();
        println!("── Phase 2: 大胆探索未知命令 (bold mode) ──");
        println!("  基于 Unix/AI/科幻/Agent 知识库迁移");
        println!();

        for entry in CURIOSITY_LIST {
            print!("  {} {} ... ", entry.command, entry.args);
            result.total_tried += 1;

            match try_command(false, entry.command, entry.args) {
                Some(output) => {
                    result.succeeded += 1;
                    let summary: String = output.lines().take(1).collect();
                    println!("✓ FOUND! {}", summary);

                    let fact = format!("意外发现: {} ({}) 存在 - {}", entry.command, entry.category, entry.reason);
                    let key = format!("explore_found_{}", entry.command.replace('-', "_"));
                    observe_and_remember(&session_id, &fact, &key, &summary, "discovery,surprise");
                    result.discoveries.push((entry.command.to_string(), summary));
                }
                None => {
                    result.failed += 1;
                    println!("✗ gap detected");
                    result.gaps.push((
                        entry.command.to_string(),
                        entry.category.to_string(),
                        entry.reason.to_string(),
                    ));

                    // Record the gap
                    let fact = format!("缺失: {} ({}) - {}", entry.command, entry.category, entry.reason);
                    let key = format!("explore_gap_{}", entry.command.replace('-', "_"));
                    observe_and_remember(&session_id, &fact, &key, entry.reason, &format!("gap,{}", entry.category));
                }
            }
        }
    }

    // ── Phase 3: Generate report ──
    let total_end = memory::now_secs();
    let duration = total_end - total_start;

    println!();
    println!("══════════════════════════════════════");
    println!("  Exploration Complete");
    println!("══════════════════════════════════════");
    println!("  Duration:    {} seconds", duration);
    println!("  Total tried: {}", result.total_tried);
    println!("  Succeeded:   {}", result.succeeded);
    println!("  Failed:      {}", result.failed);

    if bold {
        println!();
        println!("  ── Gaps Found ({}) ──", result.gaps.len());
        // Group gaps by category
        let mut by_cat: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for (cmd, cat, reason) in &result.gaps {
            by_cat.entry(cat.clone()).or_default()
                .push(format!("{} — {}", cmd, reason));
        }

        let mut cats: Vec<_> = by_cat.iter().collect();
        cats.sort_by_key(|(k, _)| *k);
        for (cat, items) in &cats {
            println!();
            println!("    [{}]", cat);
            for item in *items {
                println!("      ✗ {}", item);
            }
        }
    }

    println!();
    println!("  ── Discoveries ({}) ──", result.discoveries.len());
    for (cmd, summary) in &result.discoveries {
        println!("    ✓ {}: {}", cmd, summary);
    }

    // Write report to file
    let report_path = format!("/var/boos/explore-report-{}.txt", total_start);
    let mut report = String::new();
    report.push_str(&format!("BoOS Autonomous Exploration Report\n"));
    report.push_str(&format!("Session: {}\n", session_id));
    report.push_str(&format!("Duration: {}s\n", duration));
    report.push_str(&format!("Total: {} tried, {} succeeded, {} failed\n\n", result.total_tried, result.succeeded, result.failed));

    if bold && !result.gaps.is_empty() {
        report.push_str("=== Missing Capabilities (Gaps) ===\n");
        let mut by_cat: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for (cmd, cat, reason) in &result.gaps {
            by_cat.entry(cat.clone()).or_default()
                .push(format!("  {} — {}", cmd, reason));
        }
        let mut cats: Vec<_> = by_cat.iter().collect();
        cats.sort_by_key(|(k, _)| *k);
        for (cat, items) in &cats {
            report.push_str(&format!("\n[{}]\n", cat));
            for item in *items {
                report.push_str(&format!("{}\n", item));
            }
        }
    }

    let _ = fs::create_dir_all("/var/boos/");
    let _ = fs::write(&report_path, &report);
    println!();
    println!("  Report saved: {}", report_path);

    // End session
    let _ = memory::session_end();

    log::log("boos-explore", "completed", &[
        ("session", &log::json_escape(&session_id)),
        ("tried", &result.total_tried.to_string()),
        ("succeeded", &result.succeeded.to_string()),
        ("gaps", &result.gaps.len().to_string()),
    ]);
}
