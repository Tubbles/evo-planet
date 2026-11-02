//! Single-binary local server for the browser demo.
//!
//! Embeds the web page, worker and wasm module at compile time and serves
//! them on localhost. Run it, open the printed URL in any browser.
//!
//! Usage: evo_serve [--port 8765] [--no-open]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

const INDEX_BODY: &str = include_str!("../../../web/index.html");
const WORKER_JS: &str = include_str!("../../../web/worker.js");
const WASM_JS: &str = include_str!("../../../web/pkg/evo_wasm.js");
const WASM_BIN: &[u8] = include_bytes!("../../../web/pkg/evo_wasm_bg.wasm");

fn index_html() -> String {
    // The published artifact wraps the body in a skeleton; replicate it here.
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\"><style>[hidden]{{display:none!important}}</style></head><body>{INDEX_BODY}</body></html>"
    )
}

fn respond(mut stream: TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn handle(stream: TcpStream, index: &str) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/");
    let path = path.split('?').next().unwrap_or("/");
    // Drain headers.
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
            break;
        }
    }
    match path {
        "/" | "/index.html" => respond(stream, "200 OK", "text/html; charset=utf-8", index.as_bytes()),
        "/worker.js" => respond(stream, "200 OK", "text/javascript; charset=utf-8", WORKER_JS.as_bytes()),
        "/pkg/evo_wasm.js" => respond(stream, "200 OK", "text/javascript; charset=utf-8", WASM_JS.as_bytes()),
        "/pkg/evo_wasm_bg.wasm" => respond(stream, "200 OK", "application/wasm", WASM_BIN),
        _ => respond(stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut port: u16 = 8765;
    let mut open = true;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                port = args.get(i + 1).and_then(|p| p.parse().ok()).unwrap_or(port);
                i += 1;
            }
            "--no-open" => open = false,
            _ => {}
        }
        i += 1;
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| {
        eprintln!("could not bind port {port}: {e}");
        std::process::exit(1);
    });
    let url = format!("http://127.0.0.1:{port}/");
    println!("Evo Planet serving at {url}  (Ctrl-C to stop)");
    if open {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    let index = index_html();
    for stream in listener.incoming().flatten() {
        handle(stream, &index);
    }
}
