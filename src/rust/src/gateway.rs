use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    let line = line.trim().to_string();
    if line.is_empty() {
        return;
    }

    // Dispatch: AUTH, SESSION, or command
    let (command_line, session_id) = parse_protocol(&mut reader, &mut stream, &line, token, &peer);
    let command_line = match command_line {
        Some(c) => c,
        None => return, // connection rejected or errored
    };

    log::log("boos-gateway", "request", &[
        ("peer", &peer),
        ("command", &log::json_escape(&command_line)),
        ("session", session_id.as_deref().unwrap_or("none")),
    ]);

    // Execute via boos-exec. Set BOOS_REQUESTER=ai and optionally BOOS_SESSION
    let mut cmd = process::Command::new("/bin/boos-exec");
    cmd.env("BOOS_REQUESTER", "ai");
    if let Some(ref sid) = session_id {
        cmd.env("BOOS_SESSION", sid);
    }
    let parts: Vec<&str> = command_line.split_whitespace().collect();
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

/// Parse the gateway protocol: AUTH, SESSION, then command.
/// Returns (command_line, session_id) or None if connection should be dropped.
fn parse_protocol(
    reader: &mut BufReader<TcpStream>,
    stream: &mut TcpStream,
    first_line: &str,
    token: &Option<String>,
    peer: &str,
) -> (Option<String>, Option<String>) {
    let mut line = first_line.to_string();
    let mut session_id: Option<String> = None;
    let mut auth_done = token.is_none(); // skip auth if no token configured

    // Phase 1: handle AUTH and SESSION preamble lines
    loop {
        if !auth_done {
            if let Some(rest) = line.strip_prefix("AUTH ") {
                if rest.trim() != token.as_ref().unwrap().as_str() {
                    let _ = writeln!(stream, "AUTH FAILED");
                    log::log("boos-gateway", "auth_failed", &[("peer", peer)]);
                    return (None, None);
                }
                auth_done = true;
                // Read next line
                line.clear();
                if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                    return (None, None);
                }
                line = line.trim().to_string();
                continue;
            } else {
                let _ = writeln!(stream, "AUTH REQUIRED");
                log::log("boos-gateway", "auth_required", &[("peer", peer)]);
                return (None, None);
            }
        }

        if let Some(rest) = line.strip_prefix("SESSION ") {
            session_id = Some(rest.trim().to_string());
            // Read next line
            line.clear();
            if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                return (None, None);
            }
            line = line.trim().to_string();
            continue;
        }

        // Not AUTH or SESSION — must be the command
        break;
    }

    (Some(line), session_id)
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

    let token = Arc::new(token);
    let in_flight = Arc::new(AtomicUsize::new(0));

    // Each connection runs in its own OS thread. A bounded counter caps
    // concurrency at MAX_GATEWAY_THREADS so a burst can't fork-bomb the VM;
    // overflow connections are answered with a short "BUSY" line and closed.
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let count = in_flight.fetch_add(1, Ordering::SeqCst);
                if count >= config::MAX_GATEWAY_THREADS {
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    let _ = writeln!(s, "BUSY");
                    log::log("boos-gateway", "busy", &[
                        ("in_flight", &count.to_string()),
                    ]);
                    continue;
                }
                let tok = Arc::clone(&token);
                let counter = Arc::clone(&in_flight);
                std::thread::spawn(move || {
                    handle_connection(s, &tok);
                    counter.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(e) => {
                log::log("boos-gateway", "accept_error", &[
                    ("error", &e.to_string()),
                ]);
            }
        }
    }
}
