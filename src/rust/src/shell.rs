use std::io::{self, BufRead, Write};
use std::process::Command;

use crate::log;

pub fn main() {
    let mut stdout = io::stdout();

    // Set requester for logging
    std::env::set_var("BOOS_REQUESTER", "shell");

    writeln!(stdout, "BoOS shell started.").ok();
    writeln!(stdout, "Type 'help' to see commands.").ok();

    // Acquire and release stdin lock per-iteration so subprocesses can read stdin
    loop {
        write!(stdout, "boos> ").ok();
        stdout.flush().ok();

        let line = {
            let stdin = io::stdin();
            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => line,
                Err(_) => break,
            }
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        log::log("boos-shell", "input", &[("input", &line)]);

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "exit" | "quit" => {
                writeln!(stdout, "Goodbye.").ok();
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
            // Known commands — pass ALL args through to boos-exec
            "help" | "commands" | "status" | "log" | "caps" | "shell"
            | "poweroff" | "process" | "results" | "result" | "debug"
            | "daemons" => {
                let mut exec_args: Vec<&str> = vec![cmd];
                exec_args.extend_from_slice(args);
                let _ = Command::new("/bin/boos-exec").args(&exec_args).status();
            }
            _ => {
                writeln!(stdout, "Unknown command: {}", cmd).ok();
                writeln!(stdout, "Type 'help'.").ok();
            }
        }
    }
}
