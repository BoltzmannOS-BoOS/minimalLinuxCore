use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process;

use crate::config;
use crate::log;

/// Read the gateway auth token from env or config. If not set, auth is disabled.
fn get_auth_token() -> Option<String> {
    env::var("BOOS_GATEWAY_TOKEN").ok().or_else(|| {
        // Also check file: /etc/boos/gateway_token
        std::fs::read_to_string("/etc/boos/gateway_token")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

fn handle_connection(mut stream: TcpStream, token: &Option<String>) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "?".to_string());

    // Set read timeout so a silent client can't hang the gateway
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));

    // Clone the stream for reading so we can write to the original.
    // If clone fails (extremely rare: fd exhaustion), log and disconnect.
    let cloned = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            log::log("boos-gateway", "clone_error", &[
                ("peer", &peer),
                ("error", &e.to_string()),
            ]);
            return;
        }
    };
    let mut reader = BufReader::new(cloned);

    // Read first line
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut line = line.trim().to_string();
    if line.is_empty() {
        return;
    }

    // Auth check
    if let Some(tok) = token {
        if let Some(rest) = line.strip_prefix("AUTH ") {
            if rest.trim() != tok.as_str() {
                let _ = writeln!(stream, "AUTH FAILED");
                log::log("boos-gateway", "auth_failed", &[("peer", &peer)]);
                return;
            }
            // Auth ok, read the actual command line
            line.clear();
            if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                return;
            }
            line = line.trim().to_string();
        } else {
            // First line wasn't AUTH, but auth is required
            let _ = writeln!(stream, "AUTH REQUIRED");
            log::log("boos-gateway", "auth_required", &[("peer", &peer)]);
            return;
        }
    }

    log::log("boos-gateway", "request", &[
        ("peer", &peer),
        ("command", &log::json_escape(&line)),
    ]);

    // Execute via boos-exec. Set BOOS_REQUESTER=ai
    let mut cmd = process::Command::new("/bin/boos-exec");
    cmd.env("BOOS_REQUESTER", "ai");
    // Split the line into command + args
    let parts: Vec<&str> = line.split_whitespace().collect();
    for arg in &parts {
        cmd.arg(arg);
    }

    match cmd.output() {
        Ok(output) => {
            let _ = stream.write_all(&output.stdout);
            let _ = stream.write_all(&output.stderr);
        }
        Err(e) => {
            let _ = writeln!(stream, "Gateway error: {}", e);
        }
    }
}

pub fn main() {
    let port = env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(config::GATEWAY_DEFAULT_PORT);

    let token = get_auth_token();

    let listener = match TcpListener::bind(("0.0.0.0", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind port {}: {}", port, e);
            process::exit(1);
        }
    };

    let auth_msg = if token.is_some() { "auth enabled" } else { "auth disabled" };
    log::log("boos-gateway", "started", &[
        ("port", &port.to_string()),
        ("auth", auth_msg),
    ]);

    // Accept connections (single-threaded; sufficient for AI usage)
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let tok = token.clone();
                // Handle each connection; don't spawn thread (no tokio)
                // Use a simple process-based approach
                handle_connection(s, &tok);
            }
            Err(e) => {
                log::log("boos-gateway", "accept_error", &[
                    ("error", &e.to_string()),
                ]);
            }
        }
    }
}
