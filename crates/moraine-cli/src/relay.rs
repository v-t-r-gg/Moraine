//! Local relay process: health check, optional spawn, PID file under data dir.

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use moraine_core::{bind_from_http, MorainePaths, DEFAULT_RELAY_BIND};

const HEALTH_TIMEOUT: Duration = Duration::from_millis(400);
const START_WAIT: Duration = Duration::from_secs(5);

pub fn ensure_relay(server_http: &str, may_start: bool) -> Result<()> {
    if health_ok(server_http) {
        return Ok(());
    }
    if !may_start {
        bail!(relay_down_hint(server_http, false));
    }

    if let Some(pid) = read_pid_file() {
        if process_alive(pid) && !health_ok(server_http) {
            eprintln!("stale relay pid {pid}; will try a fresh start");
        } else if process_alive(pid) && health_ok(server_http) {
            return Ok(());
        }
    }

    eprintln!("relay not up; starting moraine-server…");
    match try_start_server(server_http)? {
        StartResult::Spawned(bin) => eprintln!("spawned {}", bin.display()),
        StartResult::Failed => bail!(relay_down_hint(server_http, true)),
    }

    let deadline = Instant::now() + START_WAIT;
    while Instant::now() < deadline {
        if health_ok(server_http) {
            eprintln!("relay ready at {server_http}");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(150));
    }
    bail!("started moraine-server but {server_http}/health still failing");
}

fn relay_down_hint(server_http: &str, tried_spawn: bool) -> String {
    let head = if tried_spawn {
        format!("could not start moraine-server; {server_http}/health is down")
    } else {
        format!("relay not reachable at {server_http}/health")
    };
    format!(
        "{head}\n\
         start it: cargo run -p moraine-server\n\
         or:       npm run server\n\
         or:       docker compose up --build"
    )
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

enum StartResult {
    Spawned(PathBuf),
    Failed,
}

fn try_start_server(server_http: &str) -> Result<StartResult> {
    let bind = bind_from_http(server_http).unwrap_or_else(|| DEFAULT_RELAY_BIND.into());

    for bin in server_binaries() {
        let mut cmd = Command::new(&bin);
        cmd.arg("--bind")
            .arg(&bind)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.spawn() {
            Ok(child) => {
                let _ = write_pid_file(child.id());
                // Detach: drop Child without wait so CLI can exit.
                std::mem::forget(child);
                return Ok(StartResult::Spawned(bin));
            }
            Err(e) => eprintln!("spawn {} failed: {e}", bin.display()),
        }
    }
    Ok(StartResult::Failed)
}

fn server_binaries() -> Vec<PathBuf> {
    let mut bins = Vec::new();
    if which_exists("moraine-server") {
        bins.push(PathBuf::from("moraine-server"));
    }
    for rel in [
        "target/debug/moraine-server",
        "target/release/moraine-server",
        "../target/debug/moraine-server",
        "../target/release/moraine-server",
    ] {
        let p = PathBuf::from(rel);
        if p.is_file() {
            bins.push(p);
        }
    }
    bins
}

fn pid_file() -> Option<PathBuf> {
    MorainePaths::default_ensure()
        .ok()
        .map(|p| p.data_dir.join("moraine-server.pid"))
}

fn write_pid_file(pid: u32) -> Result<()> {
    if let Some(path) = pid_file() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, pid.to_string())?;
    }
    Ok(())
}

fn read_pid_file() -> Option<u32> {
    let path = pid_file()?;
    let s = fs::read_to_string(path).ok()?;
    s.trim().parse().ok()
}

fn process_alive(pid: u32) -> bool {
    // signal 0: existence check (Unix)
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
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

pub fn which_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let p = dir.join(name);
                p.is_file()
            })
        })
        .unwrap_or(false)
}

pub fn find_bins(names: &[&str], rel_paths: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for n in names {
        if which_exists(n) {
            out.push(PathBuf::from(n));
        }
    }
    for rel in rel_paths {
        let p = PathBuf::from(rel);
        if p.is_file() {
            out.push(p);
        }
    }
    out
}

pub fn launch_desktop(path: &Path) -> Result<bool> {
    let abs = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let bins = find_bins(
        &["moraine-app", "moraine-desktop"],
        &[
            "target/debug/moraine-app",
            "target/release/moraine-app",
            "../target/debug/moraine-app",
            "../target/release/moraine-app",
        ],
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_relay() {
        let (h, p) = parse_host_port("http://127.0.0.1:3099").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 3099);
    }

    #[test]
    fn health_fails_when_down() {
        assert!(!health_ok("http://127.0.0.1:1"));
    }
}
