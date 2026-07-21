//! C2 smoke: version/doctor do not create project state; suite paths are coherent.

use std::process::Command;
use tempfile::tempdir;

fn moraine_bin() -> std::path::PathBuf {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/moraine");
    assert!(p.is_file(), "build moraine first");
    p
}

#[test]
fn version_json_has_build_identity() {
    let out = Command::new(moraine_bin())
        .args(["version", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["cli"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(v["build"]["schema"]["currentWritable"].as_u64().unwrap() >= 6);
    assert!(v["pathExecutables"].is_array());
}

#[test]
fn doctor_json_runs_without_project() {
    let out = Command::new(moraine_bin())
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    // doctor may exit 1 if suite not installed; still valid JSON
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["checks"].as_array().unwrap().len() >= 3);
    assert_eq!(v["build"]["product"], "Moraine");
}

#[test]
fn doctor_does_not_create_moraine_dir_in_temp() {
    let dir = tempdir().unwrap();
    let _ = Command::new(moraine_bin())
        .args(["doctor", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        !dir.path().join(".moraine").exists(),
        "doctor must not create project state"
    );
}

#[test]
fn setup_codex_dry_run_json() {
    let dir = tempdir().unwrap();
    let out = Command::new(moraine_bin())
        .args([
            "setup",
            "codex",
            "--project",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["dryRun"], true);
    assert!(!dir.path().join(".codex").exists());
}

#[test]
fn setup_codex_writes_config_and_hooks() {
    let dir = tempdir().unwrap();
    let out = Command::new(moraine_bin())
        .args([
            "setup",
            "codex",
            "--project",
            dir.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cfg = std::fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap();
    assert!(cfg.contains("[mcp_servers.moraine]"));
    assert!(cfg.contains("mcp"));
    let hooks = std::fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap();
    assert!(hooks.contains("hook-codex"));
}
