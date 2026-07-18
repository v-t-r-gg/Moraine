//! Thin helpers: relay health check and desktop launch. No process manager.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Result};
use moraine_core::DEFAULT_RELAY_BIND;

const HEALTH_TIMEOUT: Duration = Duration::from_millis(400);

pub fn require_relay(server_http: &str) -> Result<()> {
    if health_ok(server_http) {
        return Ok(());
    }
    bail!(
        "relay not reachable at {server_http}/health\n\
         start it in another terminal:\n\
           cargo run -p moraine-server\n\
           npm run server\n\
           docker compose up --build"
    );
}

pub fn health_ok(server_http: &str) -> bool {
    let Some((host, port)) = parse_host_port(server_http) else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&resolve_addr(&host, port), HEALTH_TIMEOUT)
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let req = format!("GET /health HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
    buf.contains("\"ok\"") || buf.contains("moraine-server")
}

/// Best-effort one-shot spawn (no PID tracking). Used only by `share start`.
pub fn try_spawn_server(server_http: &str) -> Result<()> {
    use moraine_core::bind_from_http;

    let bind = bind_from_http(server_http).unwrap_or_else(|| DEFAULT_RELAY_BIND.into());
    for bin in server_bins() {
        let mut cmd = Command::new(&bin);
        cmd.arg("--bind")
            .arg(&bind)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Ok(child) = cmd.spawn() {
            eprintln!("started {}", bin.display());
            std::mem::forget(child);
            return Ok(());
        }
    }
    bail!(
        "could not find/start moraine-server\n\
         build: cargo build -p moraine-server\n\
         run:   cargo run -p moraine-server"
    );
}

fn server_bins() -> Vec<PathBuf> {
    let mut bins = Vec::new();
    if which_exists("moraine-server") {
        bins.push(PathBuf::from("moraine-server"));
    }
    for rel in [
        "target/debug/moraine-server",
        "target/release/moraine-server",
    ] {
        let p = PathBuf::from(rel);
        if p.is_file() {
            bins.push(p);
        }
    }
    bins
}

pub fn launch_desktop(path: &Path) -> Result<bool> {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut bins = Vec::new();
    for n in ["moraine-app", "moraine-desktop"] {
        if which_exists(n) {
            bins.push(PathBuf::from(n));
        }
    }
    for rel in ["target/debug/moraine-app", "target/release/moraine-app"] {
        let p = PathBuf::from(rel);
        if p.is_file() {
            bins.push(p);
        }
    }
    for bin in bins {
        let mut cmd = Command::new(&bin);
        cmd.arg(&abs)
            .env("MORAINE_OPEN", &abs)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if cmd.spawn().is_ok() {
            eprintln!("opened in {}: {}", bin.display(), abs.display());
            return Ok(true);
        }
    }
    Ok(false)
}

fn which_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

fn parse_host_port(url: &str) -> Option<(String, u16)> {
    let url = url.trim();
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let hostport = rest.split('/').next().unwrap_or(rest);
    if let Some((h, p)) = hostport.split_once(':') {
        Some((h.to_string(), p.parse().ok()?))
    } else {
        let port = if url.starts_with("https://") { 443 } else { 80 };
        Some((hostport.to_string(), port))
    }
}

fn resolve_addr(host: &str, port: u16) -> SocketAddr {
    format!("{host}:{port}")
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], port)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_fails_when_down() {
        assert!(!health_ok("http://127.0.0.1:1"));
    }
}
