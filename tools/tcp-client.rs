use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;

fn main() {
    let args: Vec<String> = env::args().collect();
    let host = args.get(1).map(|s| s.as_str()).unwrap_or("127.0.0.1");
    let port = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5555u16);

    let addr = format!("{}:{}", host, port);
    let mut s = TcpStream::connect(&addr).unwrap_or_else(|e| {
        eprintln!("connect failed: {}", e);
        std::process::exit(1);
    });

    // Read all stdin and send
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    s.write_all(input.as_bytes()).ok();
    s.shutdown(std::net::Shutdown::Write).ok();

    // Read response
    let mut buf = String::new();
    s.read_to_string(&mut buf).ok();
    print!("{}", buf);
}
