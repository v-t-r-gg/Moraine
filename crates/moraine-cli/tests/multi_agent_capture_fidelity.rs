//! Controlled comparison of Codex vs Claude capture fidelity reports.
//!
//! Uses real `moraine`, `moraine-service`, Unix socket, spool processing, and
//! `moraine run coverage`. Establishes Moraine's interpretation of controlled
//! adapter events — not a universal provider-compatibility claim.

#![cfg(unix)]

use std::fs;
use std::io::Write;
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

fn ensure_service() -> PathBuf {
    // Always rebuild so fidelity tests exercise the current service code path
    // (integration pass-through, session namespaces). Stale binaries mis-attribute
    // Claude runs as codex when only the CLI binary is refreshed by cargo test.
    let root = repo_root();
    assert!(
        Command::new("cargo")
            .args(["build", "-p", "moraine-service", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success(),
        "cargo build -p moraine-service failed"
    );
    let p = root.join("target/debug/moraine-service");
    assert!(p.is_file(), "missing {}", p.display());
    p
}

fn wait_http(host_port: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(mut s) = TcpStream::connect(host_port) {
            let req =
                format!("GET /status HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
            let _ = s.write_all(req.as_bytes());
            let mut buf = String::new();
            let _ = std::io::Read::read_to_string(&mut s, &mut buf);
            if buf.contains("online") {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn write_exe(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, body).unwrap();
    let mut perm = fs::metadata(&p).unwrap().permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&p, perm).unwrap();
    p
}

fn stage_suite(temp: &Path, cli: &Path, service: &Path) -> PathBuf {
    let prefix = temp.join("suite");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::create_dir_all(prefix.join("libexec/moraine")).unwrap();
    fs::create_dir_all(prefix.join("share/moraine")).unwrap();
    fs::copy(cli, prefix.join("bin/moraine")).unwrap();
    fs::copy(service, prefix.join("libexec/moraine/moraine-service")).unwrap();
    fs::set_permissions(
        prefix.join("bin/moraine"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::set_permissions(
        prefix.join("libexec/moraine/moraine-service"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::write(
        prefix.join("share/moraine/manifest.json"),
        r#"{"product":"Moraine","version":"0.1.0","gitCommit":"t","target":"x86_64-unknown-linux-gnu","profile":"debug","schema":{"minimumReadable":3,"maximumReadable":6,"currentWritable":6},"serviceProtocolVersion":1,"mcpImplementationVersion":1,"components":{"cli":"0.1.0","service":"0.1.0","desktop":"missing"}}"#,
    )
    .unwrap();
    prefix
}

fn hook(cli: &Path, sub: &str, sock: &Path, spool: &Path, payload: &Value) {
    let mut child = Command::new(cli)
        .args([
            sub,
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
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{sub} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn wait_run(project: &Path, marker: &str) -> uuid::Uuid {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let runs = project.join(".moraine/runs");
        if runs.is_dir() {
            for ent in fs::read_dir(&runs).unwrap().flatten() {
                let p = ent.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.ends_with(".md.moraine.json") {
                    continue;
                }
                if name.contains("verification-id-direct") {
                    continue;
                }
                let raw = fs::read_to_string(&p).unwrap_or_default();
                let Ok(v) = serde_json::from_str::<Value>(&raw) else {
                    continue;
                };
                let objective = v
                    .pointer("/agent/objective")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if !objective.contains(marker) && !raw.contains(marker) {
                    continue;
                }
                if let Some(id) = v.pointer("/run/id").and_then(|x| x.as_str()) {
                    return uuid::Uuid::parse_str(id).unwrap();
                }
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!(
        "run with marker {marker} not found under {}",
        project.display()
    );
}

fn dim_obs(report: &Value, dim: &str) -> String {
    report["coverage"]["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["dimension"] == dim)
        .and_then(|d| d["observation"].as_str())
        .unwrap_or("")
        .to_string()
}

fn coverage(cli: &Path, project: &Path, run_id: uuid::Uuid) -> Value {
    let out = Command::new(cli)
        .args([
            "run",
            "coverage",
            &run_id.to_string(),
            "--project",
            project.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "coverage failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn confirm_and_checkpoint(
    cli: &Path,
    project: &Path,
    session: &str,
    run_id: uuid::Uuid,
    vid: &str,
) {
    // Confirm provisional via run start with session binding.
    let start = Command::new(cli)
        .args([
            "run",
            "start",
            "--project",
            project.to_str().unwrap(),
            "--objective",
            &format!("Moraine self-test verification_id={vid} confirmed"),
            "--idempotency-key",
            &format!("confirm-{vid}"),
            "--session-id",
            session,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "run start: stdout={} stderr={}",
        String::from_utf8_lossy(&start.stdout),
        String::from_utf8_lossy(&start.stderr)
    );
    let start_v: Value = serde_json::from_slice(&start.stdout).unwrap_or(Value::Null);
    let confirmed_id = start_v
        .pointer("/run/id")
        .and_then(|x| x.as_str())
        .unwrap_or(&run_id.to_string())
        .to_string();

    let show = Command::new(cli)
        .args([
            "run",
            "show",
            "--run-id",
            &confirmed_id,
            "--project",
            project.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        show.status.success(),
        "run show: stdout={} stderr={}",
        String::from_utf8_lossy(&show.stdout),
        String::from_utf8_lossy(&show.stderr)
    );
    let show_v: Value = serde_json::from_slice(&show.stdout).unwrap();
    let hash = show_v
        .pointer("/run/contentHash")
        .or_else(|| show_v.pointer("/contentHash"))
        .or_else(|| show_v.pointer("/packet/contentHash"))
        .and_then(|x| x.as_str())
        .unwrap_or_else(|| {
            panic!(
                "content hash missing from run show: {}",
                serde_json::to_string_pretty(&show_v).unwrap_or_default()
            )
        })
        .to_string();

    let cp_input = project.join(format!("cp-{vid}.json"));
    fs::write(
        &cp_input,
        serde_json::json!({
            "summary": "fidelity checkpoint with agent-reported evidence",
            "evidence": [{
                "kind": "note",
                "label": "agent claim for fidelity test",
                "provenance": "agent_reported"
            }]
        })
        .to_string(),
    )
    .unwrap();

    let cp = Command::new(cli)
        .args([
            "run",
            "checkpoint",
            "--run-id",
            &confirmed_id,
            "--project",
            project.to_str().unwrap(),
            "--expected-hash",
            &hash,
            "--idempotency-key",
            &format!("cp-{vid}"),
            "--input",
            cp_input.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        cp.status.success(),
        "checkpoint: stdout={} stderr={}",
        String::from_utf8_lossy(&cp.stdout),
        String::from_utf8_lossy(&cp.stderr)
    );
}

#[test]
fn codex_and_claude_fidelity_share_schema() {
    let temp = tempdir().unwrap();
    let cli = bin_moraine();
    let service = ensure_service();
    let prefix = stage_suite(temp.path(), &cli, &service);
    let staged = prefix.join("bin/moraine");
    let staged_svc = prefix.join("libexec/moraine/moraine-service");

    let spool = temp.path().join("spool");
    fs::create_dir_all(&spool).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let sock = temp.path().join("cap.sock");
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
    assert!(wait_http(&http, Duration::from_secs(8)));

    let fixtures = temp.path().join("fx");
    fs::create_dir_all(&fixtures).unwrap();
    write_exe(&fixtures, "claude", "#!/bin/sh\necho claude-fixture 0\n");
    write_exe(&fixtures, "codex", "#!/bin/sh\necho codex-fixture 0\n");

    // --- Claude ---
    let claude_proj = temp.path().join("claude-proj");
    fs::create_dir_all(&claude_proj).unwrap();
    assert!(Command::new(&staged)
        .args(["project", "init"])
        .arg(&claude_proj)
        .status()
        .unwrap()
        .success());
    {
        use moraine_provision::{adapter_for, AgentKind, VecBackupRecorder};
        let adapter = adapter_for(AgentKind::ClaudeCode);
        let plan = adapter
            .plan_install(&claude_proj, &staged)
            .expect("plan claude");
        let mut rec = VecBackupRecorder::new();
        adapter.apply(&plan, &mut rec).expect("apply claude");
    }

    let vid = uuid::Uuid::new_v4().to_string();
    let session = "fidelity-claude-1";
    hook(
        &staged,
        "hook-claude-code",
        &sock,
        &spool,
        &serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": session,
            "cwd": claude_proj.display().to_string(),
            "event_id": format!("{vid}-cs"),
        }),
    );
    hook(
        &staged,
        "hook-claude-code",
        &sock,
        &spool,
        &serde_json::json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": session,
            "cwd": claude_proj.display().to_string(),
            "event_id": format!("{vid}-cp"),
            "prompt": format!("Moraine self-test verification_id={vid}"),
        }),
    );
    let claude_run = wait_run(&claude_proj, &vid);
    confirm_and_checkpoint(&staged, &claude_proj, session, claude_run, &vid);
    hook(
        &staged,
        "hook-claude-code",
        &sock,
        &spool,
        &serde_json::json!({
            "hook_event_name": "Stop",
            "session_id": session,
            "cwd": claude_proj.display().to_string(),
            "event_id": format!("{vid}-cstop"),
        }),
    );
    // Allow stop to materialize.
    std::thread::sleep(Duration::from_millis(400));

    let claude_rep = coverage(&staged, &claude_proj, claude_run);
    if dim_obs(&claude_rep, "session_lifecycle") != "observed" {
        eprintln!(
            "claude report: {}",
            serde_json::to_string_pretty(&claude_rep).unwrap()
        );
    }
    assert_eq!(
        claude_rep["coverage"]["integration"].as_str(),
        Some("claude-code")
    );
    assert_eq!(dim_obs(&claude_rep, "session_lifecycle"), "observed");
    assert_eq!(dim_obs(&claude_rep, "prompt_activity"), "observed");
    assert_eq!(dim_obs(&claude_rep, "tool_activity"), "not_supported");
    assert_eq!(dim_obs(&claude_rep, "semantic_start"), "observed");
    assert_eq!(dim_obs(&claude_rep, "checkpoints"), "observed");
    // Checkpoint evidence is agent-reported by default.
    assert_eq!(dim_obs(&claude_rep, "agent_reported_evidence"), "observed");

    // --- Codex ---
    let codex_proj = temp.path().join("codex-proj");
    fs::create_dir_all(&codex_proj).unwrap();
    assert!(Command::new(&staged)
        .args(["project", "init"])
        .arg(&codex_proj)
        .status()
        .unwrap()
        .success());
    {
        use moraine_provision::{adapter_for, AgentKind, VecBackupRecorder};
        let adapter = adapter_for(AgentKind::Codex);
        let plan = adapter
            .plan_install(&codex_proj, &staged)
            .expect("plan codex");
        let mut rec = VecBackupRecorder::new();
        adapter.apply(&plan, &mut rec).expect("apply codex");
    }

    let vid2 = uuid::Uuid::new_v4().to_string();
    let session2 = "fidelity-codex-1";
    hook(
        &staged,
        "hook-codex",
        &sock,
        &spool,
        &serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": session2,
            "cwd": codex_proj.display().to_string(),
            "event_id": format!("{vid2}-s"),
        }),
    );
    hook(
        &staged,
        "hook-codex",
        &sock,
        &spool,
        &serde_json::json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": session2,
            "cwd": codex_proj.display().to_string(),
            "event_id": format!("{vid2}-p"),
            "prompt": format!("Moraine self-test verification_id={vid2}"),
        }),
    );
    hook(
        &staged,
        "hook-codex",
        &sock,
        &spool,
        &serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": session2,
            "cwd": codex_proj.display().to_string(),
            "event_id": format!("{vid2}-t"),
            "tool_name": "Bash",
            "tool_use_id": "call-1",
            "tool_input": { "command": "echo hi" }
        }),
    );
    let codex_run = wait_run(&codex_proj, &vid2);
    // Wait for tool event to attach to the provisional run.
    std::thread::sleep(Duration::from_millis(500));
    confirm_and_checkpoint(&staged, &codex_proj, session2, codex_run, &vid2);

    let codex_rep = coverage(&staged, &codex_proj, codex_run);
    if dim_obs(&codex_rep, "tool_activity") != "observed" {
        eprintln!(
            "codex report: {}",
            serde_json::to_string_pretty(&codex_rep).unwrap()
        );
    }
    assert_eq!(codex_rep["coverage"]["integration"].as_str(), Some("codex"));
    assert_eq!(dim_obs(&codex_rep, "session_lifecycle"), "observed");
    assert_eq!(dim_obs(&codex_rep, "prompt_activity"), "observed");
    assert_eq!(dim_obs(&codex_rep, "tool_activity"), "observed");
    assert_eq!(dim_obs(&codex_rep, "semantic_start"), "observed");
    assert_eq!(dim_obs(&codex_rep, "checkpoints"), "observed");
    assert_eq!(dim_obs(&codex_rep, "mechanical_evidence"), "observed");

    // Same schema identifiers
    let dims_c: Vec<_> = claude_rep["coverage"]["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["dimension"].as_str().unwrap().to_string())
        .collect();
    let dims_x: Vec<_> = codex_rep["coverage"]["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["dimension"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(dims_c, dims_x);
    assert!(dims_c.contains(&"session_lifecycle".to_string()));
    assert!(dims_c.contains(&"tool_activity".to_string()));
    assert!(dims_c.contains(&"semantic_start".to_string()));
    assert!(dims_c.contains(&"checkpoints".to_string()));

    let _ = child.kill();
    let _ = child.wait();
}
