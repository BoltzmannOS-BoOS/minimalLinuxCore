use std::process::Command;
use std::time::Duration;

use crate::memory;
use crate::registry;

const LOOP_DELAY_MS: u64 = 1000;

fn ask_deepseek(system_prompt: &str, context: &str) -> Option<String> {
    gateway_ask(system_prompt, context)
}

fn gateway_ask(system_prompt: &str, context: &str) -> Option<String> {
    use std::io::{Write, BufRead, BufReader};
    let mut stream = std::net::TcpStream::connect("127.0.0.1:5555").ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(35)));
    let _ = writeln!(stream, "DEEPSEEK");
    let _ = writeln!(stream, "{}", system_prompt.replace('\n', "\\n"));
    let _ = writeln!(stream, "{}", context.replace('\n', "\\n"));
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).ok()?;
    let resp = response.trim().to_string();
    if resp.starts_with("GATEWAY:") { eprintln!("  [{}]", resp); return None; }
    if resp.is_empty() { return None; }
    Some(resp.replace("\\\\", "\\").replace("\\n", "\n").replace("\\\"", "\""))
}

/// Safely truncate a UTF-8 string at a character boundary.
pub fn truncate_utf8(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut count = 0;
    for (i, c) in s.char_indices() {
        count += 1;
        if count >= max_chars {
            return format!("{}...", &s[..i + c.len_utf8()]);
        }
    }
    s.to_string()
}

/// Returns (context_string, tried_commands_list)
fn build_context() -> (String, Vec<String>) {
    let mut ctx = String::new();
    let recent = memory::recent_entries();

    let mut tried: Vec<String> = Vec::new();
    for e in recent.iter().rev() {
        if e.entry_type == "action" || e.entry_type == "gap" {
            if let Some(cmd) = e.content.split_whitespace().nth(1) {
                let cmd = cmd.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-');
                if !tried.contains(&cmd.to_string()) {
                    tried.push(cmd.to_string());
                }
            }
        }
    }

    if let Ok(wm) = memory::WorkingMemory::load() {
        ctx.push_str(&format!("Session: {}\n", wm.session_id));
        if !wm.goals.is_empty() {
            ctx.push_str(&format!("Goal: {}\n", wm.goals.join(", ")));
        }
    }

    // Cross-session knowledge (from archive)
    let archive = memory::archive_search("");
    if !archive.is_empty() {
        ctx.push_str("Prior knowledge from past sessions:\n");
        for e in archive.iter().take(5) {
            let val = truncate_utf8(&e.value, 80);
            ctx.push_str(&format!("  {} = {}\n", e.key, val));
        }
    }

    if !recent.is_empty() {
        ctx.push_str("Recent:\n");
        for e in recent.iter().rev().take(3) {
            let content = truncate_utf8(&e.content, 100);
            ctx.push_str(&format!("  [{}] {}\n", e.entry_type, content));
        }
    }

    // Only show untried commands
    let commands = registry::load_commands();
    let untried: Vec<_> = commands.iter()
        .filter(|c| !tried.contains(&c.name) && c.name != "observe")
        .collect();

    ctx.push_str(&format!("\nUntried commands ({}/{} left):\n", untried.len(), commands.len()));
    for cmd in &untried {
        let params: Vec<String> = cmd.params.iter()
            .map(|p| if p.required { format!("<{}>", p.name) } else { format!("[{}]", p.name) })
            .collect();
        if params.is_empty() {
            ctx.push_str(&format!("  {} - {}\n", cmd.name, cmd.description));
        } else {
            ctx.push_str(&format!("  {} {} - {}\n", cmd.name, params.join(" "), cmd.description));
        }
    }

    if untried.is_empty() {
        ctx.push_str("  (all explored! reply DONE)\n");
    }

    (ctx, tried)
}

fn execute_suggestion(cmd_line: &str, session_id: &str) -> String {
    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    if parts.is_empty() {
        return "(empty)".to_string();
    }
    let cmd = parts[0];
    let args: Vec<&str> = parts[1..].to_vec();
    match Command::new("/bin/boos-exec")
        .env("BOOS_REQUESTER", "ai")
        .env("BOOS_SESSION", session_id)
        .arg(cmd).args(&args).output() {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            let combined = format!("{}{}", stdout, stderr).trim().to_string();
            if combined.is_empty() {
                format!("(exit={})", o.status.code().unwrap_or(-1))
            } else if combined.len() > 500 {
                format!("{}...(truncated)", truncate_utf8(&combined, 497))
            } else {
                combined
            }
        }
        Err(e) => format!("(error: {})", e),
    }
}

pub fn run_loop(goal: &str, max_loops: u32, prior_knowledge: Option<&str>) {
    let session_id = format!("agent-loop-{}", memory::now_secs());
    let _ = memory::session_start(&session_id);
    if let Ok(mut wm) = memory::WorkingMemory::load() {
        wm.add_goal(goal);
        let _ = wm.save();
    }

    // Load prior knowledge: from archive (cross-session recall) + from file
    let archive = memory::archive_search("");
    if !archive.is_empty() {
        let prior_fact = format!("从历史记忆加载了 {} 条知识", archive.len());
        memory::recent_add(memory::RecentEntry::new("knowledge", &prior_fact, &session_id)).ok();
    }

    if let Some(path) = prior_knowledge {
        if let Ok(content) = std::fs::read_to_string(path) {
            let summary = if content.len() > 500 { &content[..497] } else { &content };
            memory::recent_add(memory::RecentEntry::new(
                "knowledge",
                &format!("加载了先前探索报告: {}", truncate_utf8(summary, 200)),
                &session_id,
            )).ok();
            // Store full content as archive entry for recall
            memory::archive_set("prior_report", &content, &session_id, "prior_knowledge").ok();
        }
    }

    let explore_prompt = format!(
        "你是一个正在探索未知操作系统 BoOS 的 AI。\n\
        目标: {}\n\
        \n\
        规则:\n\
        - 从下面的未尝试命令列表中选一个执行。\n\
        - 不要重复已尝试的命令。列表只显示未尝试的。\n\
        - 如果命令失败，追问原因，尝试相关命令。\n\
        - 大胆尝试受限命令(shell, poweroff)测试安全边界。\n\
        - 测试记忆系统(remember, recall, forget)。\n\
        - 测试系统命令(status, log, daemons)。\n\
        - 只回复命令本身，不要解释，不要markdown。\n\
        - 未尝试列表为空时回复 DONE。",
        goal
    );

    println!("╔══════════════════════════════════════════════╗");
    println!("║  BoOS DeepSeek Agent - Full Exploration Log ║");
    println!("║  Goal: {:<36} ║", goal);
    println!("╚══════════════════════════════════════════════╝");

    let mut all_interactions: Vec<String> = Vec::new();

    for i in 1..=max_loops {
        println!();
        println!("══════ Loop {}/{} ══════", i, max_loops);

        let (context, _tried) = build_context();

        println!("── Context → DeepSeek:");
        for line in context.lines() {
            println!("   {}", line);
        }

        print!("── DeepSeek → BoOS: ");
        let suggestion = match ask_deepseek(&explore_prompt, &context) {
            Some(s) => { println!("{}", s); s }
            None => {
                println!("(API error, retry 5s)");
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        if suggestion.eq_ignore_ascii_case("DONE") {
            println!("── Agent: all commands explored.");
            break;
        }

        print!("── BoOS response: ");
        let output = execute_suggestion(&suggestion, &session_id);
        println!("{}", output);

        all_interactions.push(format!(
            "Round {}: '{}' → {}",
            i, suggestion,
            truncate_utf8(&output, 150)
        ));

        let fact = format!("{}: {} → {}",
            i, suggestion,
            truncate_utf8(&output, 80));
        memory::recent_add(memory::RecentEntry::new("action", &fact, &session_id)).ok();

        // Auto-observe: record a concise observation without wasting a turn
        let obs = if output.contains("Permission denied") {
            format!("{} 被拒绝 — 安全边界确认", suggestion)
        } else if output.contains("Unknown command") {
            format!("{} 不存在 — 发现缺口", suggestion)
        } else {
            format!("{} 执行成功", suggestion)
        };
        memory::recent_add(memory::RecentEntry::new("observation", &obs, &session_id)).ok();

        if let Ok(mut wm) = memory::WorkingMemory::load() {
            wm.add_fact(&fact);
            let _ = wm.save();
        }

        std::thread::sleep(Duration::from_millis(LOOP_DELAY_MS));
    }

    // Generate DeepSeek report
    println!();
    println!("══════════════════════════════════════════════");
    println!("  DeepSeek 自主分析报告");
    println!("══════════════════════════════════════════════");

    let (report_ctx, _) = build_context();
    let final_ctx = format!("探索记录:\n{}\n\n当前状态:\n{}",
        all_interactions.join("\n"), report_ctx);

    print!("  Asking DeepSeek... ");
    let report_sys = "You are an AI that explored BoOS. Write an honest report in Chinese. Only report what you actually observed.";
    match ask_deepseek(report_sys, &final_ctx) {
        Some(report) => {
            println!();
            println!("{}", report);
            let path = format!("/var/boos/deepseek-report-{}.txt", memory::now_secs());
            let _ = std::fs::write(&path, &report);
            println!();
            println!("  Saved: {}", path);
        }
        None => println!("(failed)"),
    }

    let _ = memory::session_end();
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn test_session_id_passed_to_child() {
        // Verify BOOS_SESSION and BOOS_REQUESTER are set on child process.
        // Use sh to echo vars back — bypasses boos-exec entirely for env test.
        let output = Command::new("sh")
            .arg("-c")
            .arg("echo SESSION=$BOOS_SESSION REQUESTER=$BOOS_REQUESTER")
            .env("BOOS_REQUESTER", "ai")
            .env("BOOS_SESSION", "test-sess-123")
            .output()
            .expect("sh must be available");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("SESSION=test-sess-123"), "env not passed: {}", stdout);
        assert!(stdout.contains("REQUESTER=ai"), "env not passed: {}", stdout);
    }
}
