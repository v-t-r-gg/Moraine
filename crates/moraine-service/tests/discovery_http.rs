//! M5: live loopback HTTP discovery routes (real service binary process).
//!
//! Spawns `moraine-service` against a temp spool and exercises
//! status / projects / runs / detail / rebuild / rescan over 127.0.0.1.

use moraine_core::{init_project, run_checkpoint, run_start, CheckpointInput, RunStartRequest};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

struct Svc {
    child: Child,
    #[allow(dead_code)]
    http: String,
    #[allow(dead_code)]
    spool: PathBuf,
}

impl Drop for Svc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_http(base: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if http_get(&format!("{base}/health")).is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

fn spawn_service(spool: &Path, http_port: u16) -> Svc {
    let http = format!("127.0.0.1:{http_port}");
    let sock = spool.join("test.sock");
    let bin = env!("CARGO_BIN_EXE_moraine-service");
    let child = Command::new(bin)
        .args([
            "--spool-dir",
            spool.to_str().unwrap(),
            "--unix-socket",
            sock.to_str().unwrap(),
            "--http",
            &http,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn moraine-service");
    let base = format!("http://{http}");
    assert!(
        wait_http(&base, Duration::from_secs(8)),
        "service did not become healthy on {http}"
    );
    Svc {
        child,
        http: base,
        spool: spool.to_path_buf(),
    }
}

fn http_exchange(host_port: &str, request: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(host_port).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(3))).ok();
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                // Simple HTTP/1.0 style: stop when we have body after headers
                if let Some(pos) = find_headers_end(&buf) {
                    if let Some(cl) = content_length(&buf[..pos]) {
                        if buf.len() >= pos + cl {
                            break;
                        }
                    } else if buf.len() > pos {
                        // no content-length; wait a bit more then stop
                        thread::sleep(Duration::from_millis(30));
                        match stream.read(&mut tmp) {
                            Ok(0) | Err(_) => break,
                            Ok(n2) => buf.extend_from_slice(&tmp[..n2]),
                        }
                        break;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    String::from_utf8(buf).map_err(|e| e.to_string())
}

fn find_headers_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn content_length(headers: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(headers).ok()?;
    for line in s.lines() {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            return rest.trim().parse().ok();
        }
        // original case
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

fn http_get(url: &str) -> Result<String, String> {
    // url like http://127.0.0.1:PORT/path
    let without = url.strip_prefix("http://").ok_or("bad url")?;
    let (host_port, path) = without
        .split_once('/')
        .map(|(h, p)| (h, format!("/{p}")))
        .unwrap_or((without, "/".into()));
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
    let raw = http_exchange(host_port, &req)?;
    body_of(&raw)
}

fn http_post(url: &str) -> Result<String, String> {
    let without = url.strip_prefix("http://").ok_or("bad url")?;
    let (host_port, path) = without
        .split_once('/')
        .map(|(h, p)| (h, format!("/{p}")))
        .unwrap_or((without, "/".into()));
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host_port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let raw = http_exchange(host_port, &req)?;
    body_of(&raw)
}

fn body_of(raw: &str) -> Result<String, String> {
    if let Some(idx) = raw.find("\r\n\r\n") {
        Ok(raw[idx + 4..].to_string())
    } else {
        Err(format!("no headers in response: {raw}"))
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn setup_project(base: &Path) -> (uuid::Uuid, PathBuf, uuid::Uuid, PathBuf) {
    let project = base.join("proj");
    fs::create_dir_all(&project).unwrap();
    let init = init_project(Some(&project)).unwrap();
    let start = run_start(RunStartRequest {
        objective: "HTTP discovery live run".into(),
        idempotency_key: "http-disc-start".into(),
        project: Some(init.project_root.clone()),
        session_id: None,
    })
    .unwrap();
    run_checkpoint(
        Some(&init.project_root),
        start.run_id,
        &start.content_hash,
        "http-cp",
        CheckpointInput {
            summary: "checkpoint via http test".into(),
            actions: vec![],
            rationales: vec![],
            evidence: vec![],
            risks: vec!["r1".into()],
            open_questions: vec![],
        },
    )
    .unwrap();
    let healthy_md = start.absolute_path.clone();
    // Malformed neighbor must not suppress healthy (discovery lists; find_by_id may not).
    let runs = init.project_root.join(".moraine").join("runs");
    let bad = runs.join("zzz-bad.md");
    fs::write(&bad, "# bad\n").unwrap();
    fs::write(format!("{}.moraine.json", bad.display()), "{broken").unwrap();
    (init.project_id, init.project_root, start.run_id, healthy_md)
}

#[test]
fn discovery_routes_over_loopback_http() {
    let dir = tempdir().unwrap();
    // Place project under cwd parent so scan finds it: run service with CWD = dir
    let (project_id, _project_root, run_id, md) = setup_project(dir.path());
    let side = moraine_core::moraine_sidecar_path(&md);
    let before_md = fs::read(&md).unwrap();
    let before_side = fs::read(&side).unwrap();

    let spool = dir.path().join("spool");
    fs::create_dir_all(&spool).unwrap();
    let port = free_port();
    // Service scans std::env::current_dir for rebuild — change cwd for process via -- not available.
    // rebuild_index uses current_dir of the service process; spawn with cwd = dir.path().
    let http = format!("127.0.0.1:{port}");
    let sock = spool.join("t.sock");
    let bin = env!("CARGO_BIN_EXE_moraine-service");
    let mut child = Command::new(bin)
        .args([
            "--spool-dir",
            spool.to_str().unwrap(),
            "--unix-socket",
            sock.to_str().unwrap(),
            "--http",
            &http,
        ])
        .current_dir(dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let base = format!("http://{http}");
    assert!(wait_http(&base, Duration::from_secs(8)));

    let status: serde_json::Value =
        serde_json::from_str(&http_get(&format!("{base}/status")).unwrap()).unwrap();
    assert_eq!(status["status"], "ok");
    assert_eq!(status["online"], true);

    let rebuild: serde_json::Value =
        serde_json::from_str(&http_post(&format!("{base}/index/rebuild")).unwrap()).unwrap();
    assert_eq!(rebuild["ok"], true);
    let rev1 = rebuild["revision"].as_u64().unwrap_or(0);
    assert!(rev1 >= 1, "revision after rebuild: {rebuild}");

    let projects: serde_json::Value =
        serde_json::from_str(&http_get(&format!("{base}/projects")).unwrap()).unwrap();
    let arr = projects["projects"].as_array().expect("projects array");
    assert!(
        arr.iter()
            .any(|p| p["projectId"].as_str() == Some(&project_id.to_string())),
        "projects={projects}"
    );

    let runs: serde_json::Value =
        serde_json::from_str(&http_get(&format!("{base}/projects/{project_id}/runs")).unwrap())
            .unwrap();
    let run_list = runs["runs"].as_array().expect("runs");
    assert!(
        run_list.iter().any(
            |r| r["objective"].as_str() == Some("HTTP discovery live run")
                && r["integrity"].as_str() == Some("current")
        ),
        "healthy missing: {runs}"
    );
    assert!(
        run_list
            .iter()
            .any(|r| r["integrity"].as_str() == Some("malformed_sidecar")),
        "broken not represented: {runs}"
    );

    let detail: serde_json::Value =
        serde_json::from_str(&http_get(&format!("{base}/runs/{run_id}")).unwrap()).unwrap();
    assert!(
        detail.get("run").is_some() || detail.pointer("/run/summary").is_some(),
        "detail={detail}"
    );

    let rescan: serde_json::Value =
        serde_json::from_str(&http_post(&format!("{base}/projects/{project_id}/rescan")).unwrap())
            .unwrap();
    assert_eq!(rescan["ok"], true);
    let rev2 = rescan["revision"].as_u64().unwrap_or(0);
    assert!(rev2 > rev1, "rescan should bump revision {rev1} -> {rev2}");

    assert_eq!(
        fs::read(&md).unwrap(),
        before_md,
        "md mutated by discovery HTTP"
    );
    assert_eq!(
        fs::read(&side).unwrap(),
        before_side,
        "sidecar mutated by discovery HTTP"
    );

    let _ = child.kill();
    let _ = child.wait();
}

// Keep helper constructor referenced for compile-time binary path when other tests grow.
#[test]
fn service_binary_available() {
    let p = Path::new(env!("CARGO_BIN_EXE_moraine-service"));
    assert!(p.is_file(), "missing {}", p.display());
    let _ = free_port();
    let _ = spawn_service; // silence if unused in optimized builds
}
