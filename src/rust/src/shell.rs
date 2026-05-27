use std::io::{self, BufRead, Write};
use std::process::Command;

use crate::log;

pub fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Set requester for logging
    std::env::set_var("BOOS_REQUESTER", "shell");

    writeln!(stdout, "BoOS shell started.").ok();
    writeln!(stdout, "Type 'help' to see commands.").ok();

    let reader = stdin.lock();
    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        log::log("boos-shell", "input", &[("input", line)]);

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "exit" | "quit" => {
                let _ = Command::new("/bin/boos-exec").arg("poweroff").status();
                break;
            }
            "run" => {
                if args.is_empty() {
                    writeln!(stdout, "Usage: run <command>").ok();
                } else {
                    let _ = Command::new("/bin/boos-exec").args(args).status();
                }
            }
            "submit" => {
                if args.is_empty() {
                    writeln!(stdout, "Usage: submit <command> [args...]").ok();
                } else {
                    let mut exec_args: Vec<&str> = vec!["submit"];
                    exec_args.extend_from_slice(args);
                    let _ = Command::new("/bin/boos-exec").args(&exec_args).status();
                }
            }
            // Known commands — pass through to boos-exec
            "help" | "commands" | "status" | "log" | "caps" | "shell"
            | "poweroff" | "process" | "results" | "result" | "debug"
            | "daemons" => {
                let arg = args.first().copied().unwrap_or("");
                let _ = Command::new("/bin/boos-exec").arg(cmd).arg(arg).status();
            }
            _ => {
                writeln!(stdout, "Unknown command: {}", cmd).ok();
                writeln!(stdout, "Type 'help'.").ok();
            }
        }
    }
}
