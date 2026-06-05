use std::env;
use std::io::{BufRead, BufReader, Write, Read};
use std::net::{TcpListener, TcpStream};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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

// ── DeepSeek API proxy (gateway has key access, agent doesn't) ─────────────

fn load_api_key() -> Option<String> {
    std::fs::read_to_string("/etc/boos/agent.conf").ok().and_then(|data| {
        for line in data.lines() {
            if let Some(val) = line.trim().strip_prefix("api_key=") {
                if !val.trim().is_empty() { return Some(val.trim().to_string()); }
            }
        }
        None
    })
}

// ── FETCH: read-only network proxy (agent cannot exfiltrate) ───────────────

fn handle_fetch(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    let mut url = String::new();
    if reader.read_line(&mut url).is_err() {
        let _ = writeln!(stream, "GATEWAY: protocol error");
        return;
    }
    let url = url.trim().to_string();
    if url.is_empty() {
        let _ = writeln!(stream, "GATEWAY: empty URL");
        return;
    }
    // Defenses: HTTPS only, no localhost, strip query params
    if !url.starts_with("https://") {
        let _ = writeln!(stream, "GATEWAY: only HTTPS allowed (read-only)");
        return;
    }
    if url.contains("localhost") || url.contains("127.0.0.1") || url.contains("0.0.0.0") {
        let _ = writeln!(stream, "GATEWAY: internal addresses blocked (SSRF)");
        return;
    }
    // Strip query params to prevent data exfiltration via URL
    let clean_url = if let Some(pos) = url.find('?') { &url[..pos] } else { &url };
    // Max response size: 64KB (same as write cap)
    const MAX_FETCH_BYTES: usize = 64 * 1024;
    match ureq::get(clean_url)
        .timeout(Duration::from_secs(10))
        .call()
    {
        Ok(r) if r.status() == 200 => {
            let mut body = Vec::new();
            let mut reader = r.into_reader();
            // Read up to MAX_FETCH_BYTES
            let mut buf = [0u8; 4096];
            let mut total = 0usize;
            while total < MAX_FETCH_BYTES {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        body.extend_from_slice(&buf[..n]);
                        total += n;
                    }
                    Err(_) => break,
                }
            }
            let _ = stream.write_all(&body);
        }
        Ok(r) => { let _ = writeln!(stream, "GATEWAY: HTTP {}", r.status()); }
        Err(e) => { let _ = writeln!(stream, "GATEWAY: {}", e); }
    }
}

fn handle_deepseek(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    let mut sys = String::new();
    let mut ctx = String::new();
    if reader.read_line(&mut sys).is_err() || reader.read_line(&mut ctx).is_err() {
        let _ = writeln!(stream, "GATEWAY: protocol error");
        return;
    }
    let key = match load_api_key() {
        Some(k) => k,
        None => { let _ = writeln!(stream, "GATEWAY: no API key"); return; }
    };
    // Simple JSON escape + API call
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    let body = format!(r#"{{"model":"deepseek-chat","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"temperature":0.7,"max_tokens":500,"stream":false}}"#, esc(sys.trim()), esc(ctx.trim()));
    match ureq::post("https://api.deepseek.com/v1/chat/completions")
        .set("Authorization", &format!("Bearer {}", key))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(30))
        .send_string(&body)
    {
        Ok(r) if r.status() == 200 => {
            let mut body = String::new();
            if r.into_reader().read_to_string(&mut body).is_ok() {
                if let Some(p) = body.find("\"content\":\"") {
                    let rest = &body[p+11..]; let mut i=0; let b=rest.as_bytes();
                    while i<b.len() { if b[i]==b'\\'&&i+1<b.len(){i+=2}else if b[i]==b'"'{break}else{i+=1} }
                    let raw = &rest[..i].replace("\\n","\n").replace("\\\"","\"");
                    let _ = writeln!(stream, "{}", raw.trim());
                }
            }
        }
        Ok(r) => { let _ = writeln!(stream, "GATEWAY: HTTP {}", r.status()); }
        Err(e) => { let _ = writeln!(stream, "GATEWAY: {}", e); }
    }
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

    // Special: DEEPSEEK — gateway proxied API call (has key access)
    if command_line == "DEEPSEEK" {
        handle_deepseek(&mut stream, &mut reader);
        return;
    }

    // Special: FETCH — read-only network access (agent cannot write/exfiltrate)
    if command_line == "FETCH" {
        handle_fetch(&mut stream, &mut reader);
        return;
    }

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
