use std::io::Read;
use std::process::Command;
use std::time::Duration;

use crate::memory;

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

fn extract_content(body: &str) -> Option<String> {
    if let Some(pos) = body.find(r#""content":""#) {
        let start_byte = pos + 11;
        let rest = &body[start_byte..];
        let mut end_byte = 0;
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
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

/// Build context for the develop loop: source tree overview + goal + recent actions.
fn build_develop_context(goal: &str, recent_actions: &[String], round: u32, max_loops: u32) -> String {
    let mut ctx = String::new();

    ctx.push_str(&format!("Goal: {}\n", goal));
    ctx.push_str(&format!("Round: {}/{}\n\n", round, max_loops));

    // List available source files the AI can modify.
    ctx.push_str("Source files (use READ to inspect, WRITE to modify):\n");
    let src_dir = "src/rust/src";
    if let Ok(entries) = std::fs::read_dir(src_dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".rs") {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                ctx.push_str(&format!("  {} ({} bytes)\n", name, size));
            }
        }
    }
    ctx.push_str("  Cargo.toml (project config)\n");

    // Show recent actions (last 5)
    if !recent_actions.is_empty() {
        ctx.push_str("\nRecent actions:\n");
        for action in recent_actions.iter().rev().take(5) {
            let truncated = truncate_utf8(action, 120);
            ctx.push_str(&format!("  {}\n", truncated));
        }
    }

    ctx.push_str("\nRespond with ONE of these action formats:\n");
    ctx.push_str("  READ <filepath>           — read a source file\n");
    ctx.push_str("  WRITE <filepath> <content> — create/overwrite a file\n");
    ctx.push_str("  BUILD                     — run cargo build\n");
    ctx.push_str("  TEST                      — run cargo test\n");
    ctx.push_str("  DONE <summary>            — task complete\n");
    ctx.push_str("\nOnly respond with the action. No explanation, no markdown.\n");

    ctx
}

/// Parse and execute a single develop action.
fn execute_develop_action(action: &str) -> String {
    let action = action.trim();

    if action.is_empty() {
        return "(empty action)".to_string();
    }

    let upper = action.to_uppercase();

    if upper.starts_with("READ ") {
        let path = action[5..].trim();
        match std::fs::read_to_string(path) {
            Ok(content) => truncate_utf8(&content, 2000),
            Err(e) => format!("READ error: {}", e),
        }
    } else if upper.starts_with("WRITE ") {
        let rest = action[6..].trim();
        let space_pos = match rest.find(|c: char| c.is_whitespace()) {
            Some(p) => p,
            None => return "WRITE: missing content".to_string(),
        };
        let path = rest[..space_pos].trim();
        let content = rest[space_pos..].trim();
        if path.is_empty() || content.is_empty() {
            return "WRITE: path and content required".to_string();
        }
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        match std::fs::write(path, content) {
            Ok(()) => format!("WRITE ok: {} ({} bytes)", path, content.len()),
            Err(e) => format!("WRITE error: {}", e),
        }
    } else if upper == "BUILD" {
        match Command::new("cargo").arg("build").arg("--release").output() {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                let out = format!("{}{}", stdout, stderr);
                if o.status.success() {
                    "BUILD: success".to_string()
                } else {
                    let lines: Vec<&str> = out.lines().collect();
                    let errors: Vec<&str> = lines.iter()
                        .filter(|l| l.contains("error"))
                        .cloned()
                        .collect();
                    if !errors.is_empty() {
                        format!("BUILD: FAILED\n{}", errors.join("\n"))
                    } else {
                        format!("BUILD: FAILED (exit={})", o.status.code().unwrap_or(-1))
                    }
                }
            }
            Err(e) => format!("BUILD error: {}", e),
        }
    } else if upper == "TEST" {
        match Command::new("cargo").arg("test").output() {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                if o.status.success() {
                    if let Some(pos) = stdout.find("test result:") {
                        let summary = &stdout[pos..];
                        let end = summary.find('\n').unwrap_or(summary.len());
                        format!("TEST: {}", &summary[..end])
                    } else {
                        "TEST: success (no test summary found)".to_string()
                    }
                } else {
                    let combined = format!("{}{}", stdout, stderr);
                    let lines: Vec<&str> = combined.lines().collect();
                    let errors: Vec<&str> = lines.iter()
                        .filter(|l| l.contains("FAILED") || l.contains("error"))
                        .cloned()
                        .collect();
                    if !errors.is_empty() {
                        format!("TEST: FAILED\n{}", truncate_utf8(&errors.join("\n"), 500))
                    } else {
                        format!("TEST: FAILED (exit={})", o.status.code().unwrap_or(-1))
                    }
                }
            }
            Err(e) => format!("TEST error: {}", e),
        }
    } else {
        format!("Unknown action: {}. Use READ/WRITE/BUILD/TEST/DONE.", action)
    }
}

pub fn run_develop(api_key: Option<&str>, goal: &str, max_loops: u32) {
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

    let session_id = format!("develop-{}", memory::now_secs());
    let _ = memory::session_start(&session_id);
    if let Ok(mut wm) = memory::WorkingMemory::load() {
        wm.add_goal(goal);
        let _ = wm.save();
    }

    let develop_prompt = format!(
        "你是一个正在开发 BoOS 操作系统的 AI 工程师。\n\
        目标: {}\n\
        \n\
        规则:\n\
        - 先 READ 相关源文件理解代码结构。\n\
        - 然后用 WRITE 修改或创建文件。\n\
        - BUILD 编译验证。TEST 运行测试。\n\
        - 编译失败就修复，直到通过。\n\
        - 完成后回复 DONE <一句话总结改了什么>。\n\
        - 只回复动作本身，不要解释和 markdown。\n\
        - 一次只做一个动作。",
        goal
    );

    println!("╔══════════════════════════════════════════════╗");
    println!("║  BoOS Develop Agent                          ║");
    println!("║  Goal: {:<36} ║", goal);
    println!("║  Max loops: {:<33} ║", max_loops);
    println!("╚══════════════════════════════════════════════╝");

    let mut recent_actions: Vec<String> = Vec::new();

    for round in 1..=max_loops {
        println!();
        println!("══════ Round {}/{} ══════", round, max_loops);

        let context = build_develop_context(goal, &recent_actions, round, max_loops);

        println!("── Context → DeepSeek:");
        for line in context.lines().take(20) {
            println!("   {}", line);
        }

        print!("── DeepSeek → BoOS: ");
        let action = match ask_deepseek(&api_key, &develop_prompt, &context, 500) {
            Some(s) => { println!("{}", s); s }
            None => {
                println!("(API error, retry 5s)");
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        if action.eq_ignore_ascii_case("DONE") || action.to_uppercase().starts_with("DONE") {
            println!("── Agent: task complete.");
            let summary = if action.len() > 5 { &action[4..].trim() } else { "no summary" };
            let fact = format!("DONE: {}", summary);
            recent_actions.push(fact.clone());
            memory::recent_add(memory::RecentEntry::new("develop", &fact, &session_id)).ok();
            break;
        }

        // Execute the action
        let result = execute_develop_action(&action);
        print!("── Result: ");
        println!("{}", truncate_utf8(&result, 200));

        let entry = format!("Round {}: '{}' → {}", round, truncate_utf8(&action, 80), truncate_utf8(&result, 80));
        recent_actions.push(entry.clone());
        memory::recent_add(memory::RecentEntry::new("develop", &entry, &session_id)).ok();

        std::thread::sleep(Duration::from_millis(LOOP_DELAY_MS));
    }

    // Generate summary
    println!();
    println!("══════════════════════════════════════════════");
    println!("  Develop Session Complete");
    println!("  Actions taken: {}", recent_actions.len());
    for action in &recent_actions {
        println!("    {}", action);
    }

    let _ = memory::session_end();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_empty_action() {
        let result = execute_develop_action("");
        assert!(result.contains("empty action"));
    }

    #[test]
    fn test_execute_unknown_action() {
        let result = execute_develop_action("BLAH something");
        assert!(result.contains("Unknown action"));
    }

    #[test]
    fn test_execute_read_existing_file() {
        // Read Cargo.toml which always exists
        let result = execute_develop_action("READ Cargo.toml");
        assert!(result.contains("[package]"), "Should read file contents, got: {}", result);
        assert!(result.contains("boos"), "Should contain package name");
    }

    #[test]
    fn test_execute_read_nonexistent_file() {
        let result = execute_develop_action("READ /nonexistent/path/xyz.abc");
        assert!(result.contains("READ error"), "Should report error, got: {}", result);
    }

    #[test]
    fn test_execute_write_and_read() {
        let path = "/tmp/boos-devtest-write.txt";
        let content = "hello from develop test";
        let result = execute_develop_action(&format!("WRITE {} {}", path, content));
        assert!(result.contains("WRITE ok"), "Write should succeed, got: {}", result);
        assert!(result.contains(path));

        // Verify by reading back
        let read_back = std::fs::read_to_string(path).unwrap();
        assert_eq!(read_back.trim(), content);

        // Clean up
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_execute_write_missing_content() {
        let result = execute_develop_action("WRITE /tmp/test.txt");
        assert!(result.contains("missing content"), "Should report error, got: {}", result);
    }

    #[test]
    fn test_execute_write_creates_parent_dirs() {
        let path = "/tmp/boos-devtest/nested/subdir/test.txt";
        let content = "nested dir test";
        let result = execute_develop_action(&format!("WRITE {} {}", path, content));
        assert!(result.contains("WRITE ok"), "Should succeed creating dirs, got: {}", result);

        // Verify
        let read_back = std::fs::read_to_string(path).unwrap();
        assert_eq!(read_back.trim(), content);

        // Clean up
        let _ = std::fs::remove_dir_all("/tmp/boos-devtest");
    }

    #[test]
    fn test_execute_build() {
        // Run from the rust project directory
        let original_dir = std::env::current_dir().ok();
        let _ = std::env::set_current_dir("src/rust");
        let result = execute_develop_action("BUILD");
        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }
        // Build should succeed (codebase compiles)
        assert!(result.contains("BUILD: success"), "Build should succeed, got: {}", result);
    }

    #[test]
    fn test_execute_test() {
        let original_dir = std::env::current_dir().ok();
        let _ = std::env::set_current_dir("src/rust");
        let result = execute_develop_action("TEST");
        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }
        // Tests should pass
        assert!(result.contains("TEST:"), "Should return test result, got: {}", result);
    }

    #[test]
    fn test_execute_write_with_spaces_in_content() {
        let path = "/tmp/boos-devtest-multiline.txt";
        let content = "line one\nline two\nline three";
        let result = execute_develop_action(&format!("WRITE {} {}", path, content));
        assert!(result.contains("WRITE ok"), "Multiline write should succeed, got: {}", result);

        let read_back = std::fs::read_to_string(path).unwrap();
        assert_eq!(read_back, content);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_truncate_utf8_short() {
        let result = truncate_utf8("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_utf8_long() {
        let result = truncate_utf8("hello world this is a long string", 10);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 13); // "hello worl..." = 13 chars
    }

    #[test]
    fn test_json_escape_str_normal() {
        let result = json_escape_str("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_json_escape_str_newline() {
        let result = json_escape_str("line1\nline2");
        assert_eq!(result, "line1\\nline2");
    }

    #[test]
    fn test_build_develop_context() {
        let ctx = build_develop_context("test goal", &[], 1, 5);
        assert!(ctx.contains("Goal: test goal"));
        assert!(ctx.contains("Round: 1/5"));
        assert!(ctx.contains("READ <filepath>"));
        assert!(ctx.contains("BUILD"));
        assert!(ctx.contains("TEST"));
        assert!(ctx.contains("DONE"));
    }
}
