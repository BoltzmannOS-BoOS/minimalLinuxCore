use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::config;
use crate::gateway_policy::{
    special_protocol_allowed, validate_session_id, FetchPolicy,
};
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
    // Priority: env var (no file, no disk exposure), then fallback to file
    std::env::var("BOOS_API_KEY").ok()
        .or_else(|| {
            std::fs::read_to_string("/etc/boos/agent.conf").ok().and_then(|data| {
                for line in data.lines() {
                    if let Some(val) = line.trim().strip_prefix("api_key=") {
                        if !val.trim().is_empty() { return Some(val.trim().to_string()); }
                    }
                }
                None
            })
        })
}

fn read_protocol_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> io::Result<Option<String>> {
    let mut line = String::new();
    let bytes_read = reader
        .take(max_bytes as u64 + 1)
        .read_line(&mut line)?;
    if bytes_read == 0 {
        return Ok(None);
    }
    if bytes_read > max_bytes || !line.ends_with('\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protocol line exceeds its limit or is incomplete",
        ));
    }
    line.pop();
    if line.ends_with('\r') {
        line.pop();
    }
    Ok(Some(line))
}

// ── FETCH: explicitly allowlisted HTTPS retrieval ──────────────────────────

fn handle_fetch(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    let url = match read_protocol_line(reader, config::MAX_GATEWAY_URL_BYTES) {
        Ok(Some(url)) => url,
        _ => {
            let _ = writeln!(stream, "GATEWAY: invalid FETCH URL frame");
            return;
        }
    };
    let policy = match FetchPolicy::from_environment() {
        Ok(policy) => policy,
        Err(error) => {
            let _ = writeln!(stream, "GATEWAY: {}", error);
            return;
        }
    };
    let validated = match policy.validate_url(&url) {
        Ok(validated) => validated,
        Err(error) => {
            let _ = writeln!(stream, "GATEWAY: {}", error);
            return;
        }
    };
    if let Err(error) = policy.require_public_resolution(&validated) {
        let _ = writeln!(stream, "GATEWAY: {}", error);
        return;
    }

    const MAX_FETCH_BYTES: usize = 64 * 1024;
    match ureq::get(&validated.url)
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
                let remaining = MAX_FETCH_BYTES - total;
                let read_len = remaining.min(buf.len());
                match reader.read(&mut buf[..read_len]) {
                    Ok(0) => break,
                    Ok(n) => {
                        body.extend_from_slice(&buf[..n]);
                        total += n;
                    }
                    Err(_) => break,
                }
            }
            let tag = b"[EXTERNAL] ";
            let _ = stream.write_all(tag);
            let _ = stream.write_all(&body);
            let _ = stream.write_all(b"\n");
        }
        Ok(r) => { let _ = writeln!(stream, "GATEWAY: HTTP {}", r.status()); }
        Err(e) => { let _ = writeln!(stream, "GATEWAY: {}", e); }
    }
}

fn handle_deepseek(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    let sys = match read_protocol_line(reader, config::MAX_GATEWAY_PROMPT_BYTES) {
        Ok(Some(line)) => line,
        _ => {
            let _ = writeln!(stream, "GATEWAY: invalid system prompt frame");
            return;
        }
    };
    let ctx = match read_protocol_line(reader, config::MAX_GATEWAY_PROMPT_BYTES) {
        Ok(Some(line)) => line,
        _ => {
            let _ = writeln!(stream, "GATEWAY: invalid context frame");
            return;
        }
    };
    // Protocol escapes newlines as \n to keep single-line semantics
    let sys = sys.replace("\\n", "\n");
    let ctx = ctx.replace("\\n", "\n");
    println!("[gateway] DEEPSEEK request received");
    let key = match load_api_key() {
        Some(k) => { println!("[gateway] key found"); k }
        None => { let _ = writeln!(stream, "GATEWAY: no API key"); println!("[gateway] NO KEY"); return; }
    };
    // Simple JSON escape + API call
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    let body = format!(r#"{{"model":"deepseek-chat","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"temperature":0.7,"max_tokens":500,"stream":false}}"#, esc(&sys), esc(&ctx));
    match ureq::post("https://api.deepseek.com/v1/chat/completions")
        .set("Authorization", &format!("Bearer {}", key))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(30))
        .send_string(&body)
    {
        Ok(r) if r.status() == 200 => {
            let mut body = String::new();
            let mut response = r
                .into_reader()
                .take(config::MAX_GATEWAY_MODEL_RESPONSE_BYTES + 1);
            if response.read_to_string(&mut body).is_ok()
                && body.len() as u64 <= config::MAX_GATEWAY_MODEL_RESPONSE_BYTES
            {
                if let Some(p) = body.find("\"content\":\"") {
                    let rest = &body[p+11..]; let mut i=0; let b=rest.as_bytes();
                    while i<b.len() { if b[i]==b'\\'&&i+1<b.len(){i+=2}else if b[i]==b'"'{break}else{i+=1} }
                    let raw = &rest[..i].replace("\\\"","\"");
                    let _ = writeln!(stream, "{}", raw.trim());
                }
            } else {
                let _ = writeln!(stream, "GATEWAY: model response exceeds limit");
            }
            println!("[gateway] DEEPSEEK OK, response sent");
        }
        Ok(r) => {
            let _ = writeln!(stream, "GATEWAY: HTTP {}", r.status());
            println!("[gateway] DEEPSEEK HTTP {}", r.status());
        }
        Err(e) => {
            let _ = writeln!(stream, "GATEWAY: {}", e);
            println!("[gateway] DEEPSEEK error: {}", e);
        }
    }
}

fn handle_connection(mut stream: TcpStream, token: &Option<String>) {
    let peer_address = stream.peer_addr().ok();
    let peer = peer_address
        .map(|address| address.to_string())
        .unwrap_or_else(|| "?".to_string());
    let trusted_local = peer_address
        .map(|address| special_protocol_allowed(address.ip()))
        .unwrap_or(false);

    // Set read timeout so a silent client can't hang the gateway
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

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

    let line = match read_protocol_line(&mut reader, config::MAX_GATEWAY_COMMAND_BYTES) {
        Ok(Some(line)) => line.trim().to_string(),
        Ok(None) => return,
        Err(_) => {
            let _ = writeln!(stream, "GATEWAY: invalid command frame");
            return;
        }
    };
    if line.is_empty() {
        return;
    }

    // Dispatch: AUTH, SESSION, or command
    let (command_line, session_id) = parse_protocol(
        &mut reader,
        &mut stream,
        &line,
        token,
        &peer,
        trusted_local,
    );
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
        if !trusted_local {
            let _ = writeln!(stream, "GATEWAY: DEEPSEEK is local-only");
            log::log("boos-gateway", "special_protocol_denied", &[
                ("peer", &peer),
                ("protocol", "DEEPSEEK"),
            ]);
            return;
        }
        handle_deepseek(&mut stream, &mut reader);
        return;
    }

    // Special: FETCH — administrator-allowlisted external context retrieval.
    // GET is not intrinsically read-only or non-exfiltrating, so remote peers
    // cannot use this protocol and the destination policy defaults to deny.
    if command_line == "FETCH" {
        if !trusted_local {
            let _ = writeln!(stream, "GATEWAY: FETCH is local-only");
            log::log("boos-gateway", "special_protocol_denied", &[
                ("peer", &peer),
                ("protocol", "FETCH"),
            ]);
            return;
        }
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
    trusted_local: bool,
) -> (Option<String>, Option<String>) {
    let mut line = first_line.to_string();
    let mut session_id: Option<String> = None;
    let mut auth_done = token.is_none() || trusted_local;

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
                line = match read_protocol_line(reader, config::MAX_GATEWAY_COMMAND_BYTES) {
                    Ok(Some(line)) if !line.trim().is_empty() => line.trim().to_string(),
                    _ => return (None, None),
                };
                continue;
            } else {
                let _ = writeln!(stream, "AUTH REQUIRED");
                log::log("boos-gateway", "auth_required", &[("peer", peer)]);
                return (None, None);
            }
        }

        if let Some(rest) = line.strip_prefix("SESSION ") {
            let candidate = rest.trim();
            if validate_session_id(candidate).is_err() {
                let _ = writeln!(stream, "GATEWAY: invalid SESSION ID");
                return (None, None);
            }
            session_id = Some(candidate.to_string());
            line = match read_protocol_line(reader, config::MAX_GATEWAY_COMMAND_BYTES) {
                Ok(Some(line)) if !line.trim().is_empty() => line.trim().to_string(),
                _ => return (None, None),
            };
            continue;
        }

        // Not AUTH or SESSION — must be the command
        break;
    }

    (Some(line), session_id)
}

pub fn main() {
    // Log panics instead of silently dying
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload().downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(non-string panic)".to_string());
        let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "?".to_string());
        log::log("boos-gateway", "panic", &[("msg", &msg), ("location", &loc)]);
        eprintln!("PANIC at {}: {}", loc, msg);
    }));

    let port = env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(config::GATEWAY_DEFAULT_PORT);

    let token = get_auth_token();

    let listener = match TcpListener::bind(("0.0.0.0", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind port {}: {}", port, e);
            process::exit(config::EXIT_ERROR);
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
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        handle_connection(s, &tok);
                    }));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn protocol_line_reader_rejects_oversized_followup_lines() {
        let mut input = Cursor::new("123456789\n");

        assert!(read_protocol_line(&mut input, 8).is_err());
    }

    #[test]
    fn protocol_line_reader_returns_a_complete_line_without_newline() {
        let mut input = Cursor::new("SESSION agent-a\n");

        assert_eq!(
            read_protocol_line(&mut input, 32).unwrap().unwrap(),
            "SESSION agent-a"
        );
    }
}
