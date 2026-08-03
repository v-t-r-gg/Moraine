//! Real ProductCapture path for Claude Code using a controlled `claude` fixture.
//!
//! Stages suite binaries, configures project integration, drives hook-claude-code
//! through the Unix socket capture path, and verifies one session-bound run.

#![cfg(unix)]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bin_moraine() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_moraine"))
}

fn ensure_service_bin() -> PathBuf {
    let root = repo_root();
    let service = root.join("target/debug/moraine-service");
    if !service.is_file() {
        let st = Command::new("cargo")
            .args(["build", "-p", "moraine-service", "-q"])
            .current_dir(&root)
            .status()
            .expect("build moraine-service");
        assert!(st.success(), "cargo build -p moraine-service failed");
    }
    assert!(service.is_file(), "missing {}", service.display());
    service
}

fn wait_http(host_port: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect(host_port) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let req =
                format!("GET /status HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
            if stream.write_all(req.as_bytes()).is_ok() {
                let mut buf = String::new();
                let _ = stream.read_to_string(&mut buf);
                if buf.contains("\"online\":true") || buf.contains("\"status\":\"ok\"") {
                    return true;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn write_claude_fixture(dir: &Path) -> PathBuf {
    let path = dir.join("claude");
    fs::write(
        &path,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "claude fixture 0.0.0-test"
  exit 0
fi
echo "claude fixture: unexpected $*" >&2
exit 1
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

#[test]
fn claude_code_product_capture_socket_to_run() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("Project With Spaces");
    fs::create_dir_all(&project).unwrap();
    let spool = temp.path().join("spool");
    fs::create_dir_all(&spool).unwrap();
    let fixture_dir = temp.path().join("bin");
    fs::create_dir_all(&fixture_dir).unwrap();
    let _claude = write_claude_fixture(&fixture_dir);

    let cli = bin_moraine();
    let service = ensure_service_bin();
    assert!(cli.is_file(), "cli={}", cli.display());

    let st = Command::new(&cli)
        .args(["project", "init"])
        .arg(&project)
        .status()
        .unwrap();
    assert!(st.success());

    let prefix = temp.path().join("suite");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::create_dir_all(prefix.join("libexec/moraine")).unwrap();
    fs::create_dir_all(prefix.join("share/moraine")).unwrap();
    let staged_cli = prefix.join("bin/moraine");
    let staged_svc = prefix.join("libexec/moraine/moraine-service");
    fs::copy(&cli, &staged_cli).unwrap();
    fs::copy(&service, &staged_svc).unwrap();
    fs::set_permissions(&staged_cli, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&staged_svc, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        prefix.join("share/moraine/manifest.json"),
        r#"{
  "product": "Moraine",
  "version": "0.1.0",
  "gitCommit": "test",
  "target": "x86_64-unknown-linux-gnu",
  "profile": "debug",
  "schema": { "minimumReadable": 3, "maximumReadable": 6, "currentWritable": 6 },
  "serviceProtocolVersion": 1,
  "mcpImplementationVersion": 1,
  "components": { "cli": "0.1.0", "service": "0.1.0", "desktop": "missing" }
}"#,
    )
    .unwrap();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let sock = temp.path().join("moraine.sock");
    let http = format!("127.0.0.1:{port}");

    let mut child = Command::new(&staged_svc)
        .args([
            "--http",
            &http,
            "--unix-socket",
            sock.to_str().unwrap(),
            "--spool-dir",
            spool.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    assert!(
        wait_http(&http, Duration::from_secs(8)),
        "service did not become ready on {http}"
    );

    let path_env = format!(
        "{}:{}:{}",
        fixture_dir.display(),
        prefix.join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let integrate = Command::new(&staged_cli)
        .args([
            "integrate",
            "claude-code",
            "--project",
            project.to_str().unwrap(),
            "--json",
        ])
        .env("MORAINE_PREFIX", &prefix)
        .env("MORAINE_CLAUDE_CODE", fixture_dir.join("claude"))
        .env("PATH", &path_env)
        .env("MORAINE_SOCKET", &sock)
        .env("MORAINE_SPOOL_DIR", &spool)
        .output()
        .unwrap();
    assert!(
        integrate.status.success(),
        "integrate failed: stdout={} stderr={}",
        String::from_utf8_lossy(&integrate.stdout),
        String::from_utf8_lossy(&integrate.stderr)
    );

    let mcp: Value =
        serde_json::from_str(&fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert!(mcp["mcpServers"]["moraine"]["moraineManaged"]
        .as_bool()
        .unwrap_or(false));
    assert_eq!(mcp["mcpServers"]["moraine"]["args"][0], "mcp");

    let settings: Value =
        serde_json::from_str(&fs::read_to_string(project.join(".claude/settings.json")).unwrap())
            .unwrap();
    for event in ["SessionStart", "UserPromptSubmit", "Stop"] {
        let arr = settings["hooks"][event]
            .as_array()
            .unwrap_or_else(|| panic!("missing hooks for {event}"));
        assert!(
            arr.iter().any(|g| {
                g.get("hooks")
                    .and_then(|h| h.as_array())
                    .into_iter()
                    .flatten()
                    .any(|h| {
                        h.get("command")
                            .and_then(|c| c.as_str())
                            .map(|c| c.contains("hook-claude-code"))
                            .unwrap_or(false)
                    })
            }),
            "missing managed hook for {event}"
        );
    }

    let session = "claude-product-session-1";
    let verification_id = uuid::Uuid::new_v4().to_string();
    for (event, prompt) in [
        ("SessionStart", None),
        (
            "UserPromptSubmit",
            Some(format!(
                "Moraine self-test verification_id={verification_id}"
            )),
        ),
        ("Stop", None),
    ] {
        let mut payload = serde_json::json!({
            "hook_event_name": event,
            "session_id": session,
            "cwd": project.display().to_string(),
            "event_id": format!("claude-pc-{verification_id}-{event}"),
        });
        if let Some(p) = prompt {
            payload["prompt"] = serde_json::json!(p);
        }
        let mut hook = Command::new(&staged_cli)
            .args([
                "hook-claude-code",
                "--socket",
                sock.to_str().unwrap(),
                "--spool-dir",
                spool.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        hook.stdin
            .as_mut()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        let out = hook.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "hook {event} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let namespaced = format!("claude-code:{session}");
    // Spool processor interval is 5s; allow two cycles plus delivery.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut found = false;
    while Instant::now() < deadline {
        let runs_dir = project.join(".moraine/runs");
        if runs_dir.is_dir() {
            for ent in fs::read_dir(&runs_dir).unwrap().flatten() {
                let p = ent.path();
                if p.extension().and_then(|e| e.to_str()) == Some("md") {
                    let text = fs::read_to_string(&p).unwrap_or_default();
                    if text.contains(&verification_id) {
                        found = true;
                        break;
                    }
                }
            }
        }
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if !found {
        let pending: Vec<_> = fs::read_dir(&spool)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let failed: Vec<_> = fs::read_dir(spool.join("failed"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let processed: Vec<_> = fs::read_dir(spool.join("processed"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        panic!(
            "expected discoverable run with verification_id={verification_id} session={namespaced}; pending={pending:?} processed={processed:?} failed={failed:?}"
        );
    }

    let mut mcp = mcp;
    mcp["mcpServers"]["keep"] = serde_json::json!({"command": "keep-me"});
    fs::write(
        project.join(".mcp.json"),
        serde_json::to_string_pretty(&mcp).unwrap(),
    )
    .unwrap();

    let remove = Command::new(&staged_cli)
        .args([
            "integrate",
            "claude-code",
            "--project",
            project.to_str().unwrap(),
            "--remove",
            "--json",
        ])
        .env("MORAINE_PREFIX", &prefix)
        .output()
        .unwrap();
    assert!(
        remove.status.success(),
        "remove failed: {}",
        String::from_utf8_lossy(&remove.stderr)
    );
    let after: Value =
        serde_json::from_str(&fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert!(after["mcpServers"].get("moraine").is_none());
    assert!(after["mcpServers"]["keep"].is_object());
    assert!(project.join(".moraine").is_dir());

    let _ = child.kill();
    let _ = child.wait();
}
