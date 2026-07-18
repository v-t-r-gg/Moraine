//! Integration: share CLI against a live moraine-server.

use std::fs;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

fn server_bin() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/moraine-server"),
        PathBuf::from("target/debug/moraine-server"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_moraine"))
}

fn free_bind() -> (String, String) {
    for port in [3199u16, 3299, 3399] {
        let bind = format!("127.0.0.1:{port}");
        if TcpStream::connect_timeout(&bind.parse().unwrap(), Duration::from_millis(50)).is_err() {
            return (bind, format!("http://127.0.0.1:{port}"));
        }
    }
    ("127.0.0.1:3199".into(), "http://127.0.0.1:3199".into())
}

#[test]
fn share_fails_when_relay_down() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("x.md");
    fs::write(&path, "x\n").unwrap();
    let out = Command::new(cli_bin())
        .args([
            "share",
            path.to_str().unwrap(),
            "--json",
            "--server",
            "http://127.0.0.1:1",
        ])
        .output()
        .expect("run share");
    assert_eq!(out.status.code(), Some(3));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json err");
    assert_eq!(v["ok"], false);
    assert_eq!(v["code"], 3);
}

#[test]
fn status_json_for_path_is_readonly() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("note.md");
    fs::write(&path, "# hi\n").unwrap();
    // Legacy sidecar: status must read counts without migrating or creating ledger
    let side = format!("{}.comments.json", path.display());
    fs::write(
        &side,
        r#"{"version":1,"comments":[{"id":"00000000-0000-4000-8000-000000000001","body":"n","author":"A","quote":"hi","createdAt":"2020-01-01T00:00:00Z","resolved":false,"kind":"suggestion"}]}"#,
    )
    .unwrap();

    let out = Command::new(cli_bin())
        .args(["status", path.to_str().unwrap(), "--json"])
        .output()
        .expect("status");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert!(v["room"].as_str().unwrap().starts_with("doc_"));
    assert_eq!(v["annotations"]["suggestionsOpen"], 1);
    assert_eq!(v["run"]["initialized"], false);
    assert!(v["run"]["id"].is_null());
    assert_eq!(v["run"]["reviewState"], "unreviewed");
    assert_eq!(v["run"]["decisionCurrent"], true);
    assert_eq!(v["review"]["decisionCount"], 0);
    assert!(v["run"]["contentHash"].as_str().unwrap().len() == 64);
    // Status must not create .moraine.json or archive legacy
    assert!(!PathBuf::from(format!("{}.moraine.json", path.display())).exists());
    assert!(PathBuf::from(&side).exists());
}

#[test]
fn init_creates_ledger() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("init.md");
    fs::write(&path, "body\n").unwrap();
    let out = Command::new(cli_bin())
        .args(["init", path.to_str().unwrap(), "--json"])
        .output()
        .expect("init");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["run"]["initialized"], true);
    assert!(v["run"]["id"].as_str().unwrap().len() > 8);
    assert!(PathBuf::from(format!("{}.moraine.json", path.display())).exists());

    let st = Command::new(cli_bin())
        .args(["status", path.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let s: serde_json::Value = serde_json::from_slice(&st.stdout).unwrap();
    assert_eq!(s["run"]["initialized"], true);
    assert_eq!(s["run"]["id"], v["run"]["id"]);
}

#[test]
fn decide_and_stale_after_edit() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("run.md");
    fs::write(&path, "v1\n").unwrap();

    let out = Command::new(cli_bin())
        .args([
            "decide",
            path.to_str().unwrap(),
            "--decision",
            "approved",
            "--reviewer",
            "Ada",
            "--reason",
            "looks good",
            "--json",
        ])
        .output()
        .expect("decide");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["run"]["reviewState"], "approved");
    assert_eq!(v["run"]["decisionCurrent"], true);
    assert_eq!(v["review"]["decisionCount"], 1);
    assert_eq!(v["review"]["latestDecision"]["decision"], "approved");
    assert_eq!(v["review"]["latestDecision"]["reviewerLabel"], "Ada");
    let run_id = v["run"]["id"].as_str().unwrap().to_string();
    let hash1 = v["run"]["contentHash"].as_str().unwrap().to_string();

    // Content change -> stale
    fs::write(&path, "v2\n").unwrap();
    let st = Command::new(cli_bin())
        .args(["status", path.to_str().unwrap(), "--json"])
        .output()
        .expect("status after edit");
    assert!(st.status.success());
    let s: serde_json::Value = serde_json::from_slice(&st.stdout).unwrap();
    assert_eq!(s["run"]["id"], run_id);
    assert_eq!(s["run"]["reviewState"], "stale");
    assert_eq!(s["run"]["decisionCurrent"], false);
    assert_ne!(s["run"]["contentHash"].as_str().unwrap(), hash1);
    assert_eq!(s["review"]["decisionCount"], 1);
    assert_eq!(s["review"]["latestDecision"]["contentHash"], hash1);

    // New decision supersedes
    let out2 = Command::new(cli_bin())
        .args([
            "decide",
            path.to_str().unwrap(),
            "--decision",
            "changes_requested",
            "--reviewer",
            "Bob",
            "--json",
        ])
        .output()
        .expect("decide2");
    assert!(out2.status.success());
    let v2: serde_json::Value = serde_json::from_slice(&out2.stdout).unwrap();
    assert_eq!(v2["run"]["reviewState"], "changes_requested");
    assert_eq!(v2["run"]["decisionCurrent"], true);
    assert_eq!(v2["review"]["decisionCount"], 2);
    assert_eq!(v2["run"]["id"], run_id);

    // Invalid decision
    let bad = Command::new(cli_bin())
        .args([
            "decide",
            path.to_str().unwrap(),
            "--decision",
            "maybe",
            "--reviewer",
            "X",
            "--json",
        ])
        .output()
        .expect("bad decide");
    assert_eq!(bad.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&bad.stdout).unwrap();
    assert_eq!(err["ok"], false);

    // Expected-hash mismatch
    let conflict = Command::new(cli_bin())
        .args([
            "decide",
            path.to_str().unwrap(),
            "--decision",
            "approved",
            "--reviewer",
            "Z",
            "--expected-hash",
            "0".repeat(64).as_str(),
            "--json",
        ])
        .output()
        .expect("conflict decide");
    assert_eq!(conflict.status.code(), Some(1));
    let c: serde_json::Value = serde_json::from_slice(&conflict.stdout).unwrap();
    assert_eq!(c["ok"], false);
    assert_eq!(c["error"]["kind"], "document_revision_conflict");
}

#[test]
fn info_json() {
    let out = Command::new(cli_bin())
        .args(["info", "--json"])
        .output()
        .expect("info");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["name"], "moraine");
}

#[test]
fn share_json_with_running_server() {
    let Some(server) = server_bin() else {
        eprintln!("skip: build moraine-server first");
        return;
    };

    let (bind, base) = free_bind();
    let mut child = Command::new(&server)
        .args(["--bind", &bind])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server");

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&bind.parse().unwrap(), Duration::from_millis(100)).is_ok() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let dir = tempdir().unwrap();
    let path = dir.path().join("shared.md");
    fs::write(&path, "# share test\n").unwrap();

    let out = Command::new(cli_bin())
        .args([
            "share",
            path.to_str().unwrap(),
            "--json",
            "--server",
            &base,
            "--ui",
            "http://localhost:1420",
        ])
        .output()
        .expect("run share");

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(v["room"].as_str().unwrap().starts_with("doc_"));
    assert!(v["url"].as_str().unwrap().contains("room="));
}
