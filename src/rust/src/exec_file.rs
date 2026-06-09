use std::fs;
use std::path::Path;
use std::process;

use crate::config::{self, EXIT_ALLOWED, EXIT_DENIED, EXIT_ERROR};

/// Handle file-operation builtins. Returns Some(exit_code) if matched, None if not.
pub fn run_file_builtin(exec_target: &str, args: &str) -> Option<i32> {
    match exec_target {
        "__builtin_read_file" => {
            let path = args.trim();
            if path.is_empty() {
                eprintln!("Usage: read-file <path>");
                return Some(EXIT_ERROR);
            }
            if config::is_protected_read_path(path) {
                eprintln!("read-file: '{}' is protected from read access", path);
                return Some(EXIT_DENIED);
            }
            match fs::read_to_string(path) {
                Ok(content) => {
                    println!("{}", content);
                    Some(EXIT_ALLOWED)
                }
                Err(e) => {
                    eprintln!("read-file: {}", e);
                    Some(EXIT_ERROR)
                }
            }
        }
        "__builtin_exec" => {
            let args_trimmed = args.trim();
            if args_trimmed.is_empty() {
                eprintln!("Usage: exec <binary> [args...]");
                return Some(EXIT_ERROR);
            }
            let parts: Vec<&str> = args_trimmed.split_whitespace().collect();
            let cmd = parts[0];
            let full_cmd = args_trimmed;
            let allowed = config::EXEC_ALLOWLIST.iter().any(|prefix| full_cmd.starts_with(prefix));
            if !allowed {
                eprintln!("exec: '{}' is not in the exec allowlist (BIOS restriction)", full_cmd);
                return Some(EXIT_DENIED);
            }
            let cmd_args = &parts[1..];
            match process::Command::new(cmd).args(cmd_args).status() {
                Ok(s) => Some(s.code().unwrap_or(EXIT_ERROR)),
                Err(e) => {
                    eprintln!("exec: {}", e);
                    Some(EXIT_ERROR)
                }
            }
        }
        "__builtin_write_file" => {
            let args_trimmed = args.trim();
            if args_trimmed.is_empty() {
                eprintln!("Usage: write-file <path> <content>");
                return Some(EXIT_ERROR);
            }
            let space_pos = match args_trimmed.find(|c: char| c.is_whitespace()) {
                Some(p) => p,
                None => {
                    eprintln!("Usage: write-file <path> <content>");
                    return Some(EXIT_ERROR);
                }
            };
            let path = args_trimmed[..space_pos].trim();
            let content = args_trimmed[space_pos..].trim();
            if path.is_empty() || content.is_empty() {
                eprintln!("Usage: write-file <path> <content>");
                return Some(EXIT_ERROR);
            }
            if config::is_protected_path(path) {
                eprintln!("write-file: '{}' is a protected system path (BIOS restriction)", path);
                return Some(EXIT_DENIED);
            }
            if let Some(parent) = Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = fs::create_dir_all(parent);
                }
            }
            match fs::write(path, content) {
                Ok(()) => {
                    println!("Written: {} ({} bytes)", path, content.len());
                    Some(EXIT_ALLOWED)
                }
                Err(e) => {
                    eprintln!("write-file: {}", e);
                    Some(EXIT_ERROR)
                }
            }
        }
        "__builtin_list_dir" => {
            let path = args.trim();
            let dir_path = if path.is_empty() { "." } else { path };
            match fs::read_dir(dir_path) {
                Ok(entries) => {
                    let mut list: Vec<_> = entries.filter_map(|e| e.ok()).collect();
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
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        println!("  {} {:>8} {}", file_type, size, name);
                    }
                    println!("  ({} entries)", list.len());
                    Some(EXIT_ALLOWED)
                }
                Err(e) => {
                    eprintln!("list-dir: {}", e);
                    Some(EXIT_ERROR)
                }
            }
        }
        "__builtin_stat" => {
            let path = args.trim();
            if path.is_empty() {
                eprintln!("Usage: stat <path>");
                return Some(EXIT_ERROR);
            }
            match fs::metadata(path) {
                Ok(m) => {
                    let ftype = if m.is_dir() { "directory" }
                        else if m.is_symlink() { "symlink" }
                        else if m.is_file() { "file" }
                        else { "other" };
                    println!("File: {}", path);
                    println!("  Type: {}", ftype);
                    println!("  Size: {} bytes", m.len());
                    use std::os::unix::fs::PermissionsExt;
                    println!("  Permissions: {:o}", m.permissions().mode() & 0o777);
                    if let Ok(mtime) = m.modified() {
                        if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                            println!("  Modified: {} (epoch)", dur.as_secs());
                        }
                    }
                    Some(EXIT_ALLOWED)
                }
                Err(e) => {
                    eprintln!("stat: {}", e);
                    Some(EXIT_ERROR)
                }
            }
        }
        _ => None,
    }
}
