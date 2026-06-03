use std::io::Read;
use std::process::Command;
use std::time::Duration;

use crate::memory;
use crate::registry;

const DEEPSEEK_API: &str = "https://api.deepseek.com/v1/chat/completions";
const LOOP_DELAY_MS: u64 = 1000;
const API_KEY_FILE: &str = "/etc/boos/agent.conf";

fn load_api_key() -> Option<String> {
    if let Ok(data) = std::fs::read_to_string(API_KEY_FILE) {
        for line in data.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("api_key=") {
                let key = val.trim();
                if !key.is_empty() {
                    return Some(key.to_string());
                }
            }
        }
    }
    None
}

fn json_escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn extract_content(body: &str) -> Option<String> {
    // Find "content":"
    if let Some(pos) = body.find(r#""content":""#) {
        let start_byte = pos + 11; // len of "content":"
        let rest = &body[start_byte..];
        // Find closing unescaped quote, handling UTF-8 properly
        let mut end_byte = 0;
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2; // skip escaped char
            } else if bytes[i] == b'"' {
                end_byte = i;
                break;
            } else {
                i += 1;
            }
        }
        let raw = &rest[..end_byte];
        let content = String::from_utf8_lossy(raw.as_bytes()).to_string()
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\");
        Some(content.trim().to_string())
    } else {
        None
    }
}

fn ask_deepseek(api_key: &str, system_prompt: &str, context: &str, max_tokens: u32) -> Option<String> {
    let escaped_system = json_escape_str(system_prompt);
    let escaped_user = json_escape_str(context);
    let body = format!(
        r#"{{"model":"deepseek-chat","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"temperature":0.7,"max_tokens":{},"stream":false}}"#,
        escaped_system, escaped_user, max_tokens
    );

    let response = ureq::post(DEEPSEEK_API)
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(30))
        .send_string(&body);

    match response {
        Ok(resp) => {
            let status = resp.status();
            let mut resp_body = String::new();
            if resp.into_reader().read_to_string(&mut resp_body).is_ok() {
                if status != 200 {
                    eprintln!("  [HTTP {}] {}", status, truncate_utf8(&resp_body, 200));
                    return None;
                }
                extract_content(&resp_body)
            } else {
                eprintln!("  [read error]");
                None
            }
        }
        Err(e) => {
            eprintln!("  [API error: {}]", e);
            None
        }
    }
}

/// Safely truncate a UTF-8 string at a character boundary.
fn truncate_utf8(s: &str, max_chars: usize) -> String {
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

fn execute_suggestion(cmd_line: &str) -> String {
    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    if parts.is_empty() {
        return "(empty)".to_string();
    }
    let cmd = parts[0];
    let args: Vec<&str> = parts[1..].to_vec();
    match Command::new("/bin/boos-exec").arg(cmd).args(&args).output() {
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

pub fn run_loop(api_key: Option<&str>, goal: &str, max_loops: u32, prior_knowledge: Option<&str>) {
    let api_key = match api_key {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => match load_api_key() {
            Some(k) => k,
            None => {
                eprintln!("No API key. Set api_key=sk-xxx in {}", API_KEY_FILE);
                return;
            }
        }
    };

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
        let suggestion = match ask_deepseek(&api_key, &explore_prompt, &context, 300) {
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
        let output = execute_suggestion(&suggestion);
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

    let _report_prompt = format!(
        "你刚完成了 BoOS 探索。以下是探索记录:\n{}\n\n\
         写一份中文报告:\n\
         1. BoOS 是什么?\n\
         2. 它能够做什么? (只写实际观察到的)\n\
         3. 它不能做什么? (发现的缺口)\n\
         4. 什么让你意外?\n\
         5. 下一步应该加什么功能?",
        all_interactions.join("\n")
    );

    let _report_sys = "你是一个刚刚探索了 BoOS 的 AI。写一份诚实的中文报告。只报告你实际观察到的。";
    let (report_ctx, _) = build_context();
    let final_ctx = format!("探索记录:\n{}\n\n当前状态:\n{}",
        all_interactions.join("\n"), report_ctx);

    print!("  Asking DeepSeek... ");
    let report_sys = "You are an AI that explored BoOS. Write an honest report in Chinese. Only report what you actually observed.";
    match ask_deepseek(&api_key, report_sys, &final_ctx, 2000) {
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
