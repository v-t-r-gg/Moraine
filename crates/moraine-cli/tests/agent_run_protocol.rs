//! CLI integration for agent run protocol.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_moraine"))
}

fn run_json(args: &[&str]) -> (i32, serde_json::Value, String) {
    let out = Command::new(cli_bin())
        .args(args)
        .output()
        .expect("run moraine");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let code = out.status.code().unwrap_or(1);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!("expected JSON stdout code={code} stderr={stderr} stdout={stdout}")
    });
    (code, v, stderr)
}

#[test]
fn project_init_and_run_lifecycle_cli() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_str().unwrap();

    let (code, v, _) = run_json(&["project", "init", root, "--json"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["ok"], true);
    assert_eq!(v["project"]["created"], true);
    let project_id = v["project"]["id"].as_str().unwrap().to_string();

    let (code, v, _) = run_json(&["project", "init", root, "--json"]);
    assert_eq!(code, 0);
    assert_eq!(v["project"]["created"], false);
    assert_eq!(v["project"]["id"], project_id);
    assert!(!dir.path().join(".gitignore").exists());

    let (code, start, _) = run_json(&[
        "run",
        "start",
        "--objective",
        "CLI protocol dogfood",
        "--idempotency-key",
        "cli-start-1",
        "--project",
        root,
        "--json",
    ]);
    assert_eq!(code, 0, "{start}");
    assert_eq!(start["ok"], true);
    let run_id = start["run"]["id"].as_str().unwrap().to_string();
    let hash = start["run"]["contentHash"].as_str().unwrap().to_string();
    let path = start["run"]["recordPath"].as_str().unwrap().to_string();
    assert!(path.starts_with(".moraine/runs/"));
    assert!(start["run"].get("markdown").is_none());

    // size bound for start
    let packed = serde_json::to_vec(&start).unwrap();
    assert!(packed.len() < 2048, "start response {}", packed.len());

    let abs = dir.path().join(&path);
    assert!(abs.is_file());
    let md = fs::read_to_string(&abs).unwrap();
    assert!(md.contains("## Human notes"));
    // external human edit
    let md2 = md.replace(
        "## Human notes\n\n",
        "## Human notes\n\nReviewer note: looks good so far.\n",
    );
    fs::write(&abs, &md2).unwrap();
    let (code, st, _) = run_json(&["status", abs.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0, "{st}");
    let hash2 = st["run"]["contentHash"].as_str().unwrap().to_string();

    let cp_path = dir.path().join("cp.json");
    fs::write(
        &cp_path,
        r#"{"summary":"First checkpoint","actions":["touched protocol"],"rationales":[],"evidence":[{"kind":"command_result","label":"unit","command":"cargo test -p moraine-core","exitCode":0,"provenance":"agent_reported"}],"risks":[],"openQuestions":[]}"#,
    )
    .unwrap();

    let (code, cp, _) = run_json(&[
        "run",
        "checkpoint",
        "--run-id",
        &run_id,
        "--expected-hash",
        &hash2,
        "--idempotency-key",
        "cli-cp-1",
        "--input",
        cp_path.to_str().unwrap(),
        "--project",
        root,
        "--json",
    ]);
    assert_eq!(code, 0, "{cp}");
    let hash3 = cp["run"]["contentHash"].as_str().unwrap().to_string();
    let md3 = fs::read_to_string(&abs).unwrap();
    assert!(md3.contains("Reviewer note"));
    assert!(md3.contains("First checkpoint"));
    assert!(md3.contains("agent_reported"));

    // stale hash fails
    let (code, err, _) = run_json(&[
        "run",
        "checkpoint",
        "--run-id",
        &run_id,
        "--expected-hash",
        &hash,
        "--idempotency-key",
        "cli-cp-stale",
        "--input",
        cp_path.to_str().unwrap(),
        "--project",
        root,
        "--json",
    ]);
    assert_eq!(code, 1);
    assert_eq!(err["ok"], false);
    assert_eq!(err["error"]["code"], "revision_conflict");

    let (code, show, _) = run_json(&[
        "run",
        "show",
        "--run-id",
        &run_id,
        "--project",
        root,
        "--json",
    ]);
    assert_eq!(code, 0, "{show}");
    assert!(show["run"]["markdown"].is_null() || show["run"].get("markdown").is_none());
    assert_eq!(show["run"]["checkpointCount"], 1);
    let show_bytes = serde_json::to_vec(&show).unwrap();
    assert!(show_bytes.len() < 4096, "show size {}", show_bytes.len());

    let (code, ready, _) = run_json(&[
        "run",
        "ready",
        "--run-id",
        &run_id,
        "--expected-hash",
        &hash3,
        "--idempotency-key",
        "cli-ready-1",
        "--summary",
        "CLI path done",
        "--project",
        root,
        "--json",
    ]);
    assert_eq!(code, 0, "{ready}");
    assert_eq!(ready["run"]["state"], "ready_for_review");
    let hash4 = ready["run"]["contentHash"].as_str().unwrap().to_string();

    // human decide
    let (code, dec, _) = run_json(&[
        "decide",
        abs.to_str().unwrap(),
        "--decision",
        "approved",
        "--reviewer",
        "cli-tester",
        "--expected-hash",
        &hash4,
        "--json",
    ]);
    assert_eq!(code, 0, "{dec}");

    let (code, resumed, _) = run_json(&[
        "run",
        "resume",
        "--run-id",
        &run_id,
        "--expected-hash",
        &hash4,
        "--idempotency-key",
        "cli-resume-1",
        "--reason",
        "one more fix",
        "--project",
        root,
        "--json",
    ]);
    assert_eq!(code, 0, "{resumed}");
    assert_eq!(resumed["run"]["state"], "active");
    assert_eq!(resumed["run"]["reviewState"], "stale");

    // open resolves path (may not launch desktop in CI)
    let (code, open, _) = run_json(&[
        "run",
        "open",
        "--run-id",
        &run_id,
        "--project",
        root,
        "--json",
    ]);
    // launched may be false; still ok:true with path when binary missing? currently EXIT_ERR if not launched in non-json; with json we still return ok true with launched false
    // Looking at our code: if !launched && !json return EXIT_ERR; with json we always EXIT_OK with launched false
    assert_eq!(code, 0, "{open}");
    assert_eq!(open["ok"], true);
    assert!(open["run"]["path"].as_str().unwrap().contains(".md"));
}

#[test]
fn start_auto_inits_without_project_init() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let (code, start, _) = run_json(&[
        "run",
        "start",
        "--objective",
        "auto init",
        "--idempotency-key",
        "auto-1",
        "--project",
        root,
        "--json",
    ]);
    assert_eq!(code, 0, "{start}");
    assert!(dir.path().join(".moraine/project.json").is_file());
}
