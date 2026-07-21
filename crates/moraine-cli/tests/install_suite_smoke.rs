//! C2 smoke: version/doctor/setup; suite paths; Codex merge safety.

use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn moraine_bin() -> std::path::PathBuf {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/moraine");
    assert!(p.is_file(), "build moraine first");
    p
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(moraine_bin()).args(args).output().unwrap()
}

#[test]
fn version_json_has_build_identity() {
    let out = run(&["version", "--json"]);
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
    let out = run(&["doctor", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["checks"].as_array().unwrap().len() >= 3);
    assert_eq!(v["build"]["product"], "Moraine");
    // statuses use pass/warn/fail/info
    for c in v["checks"].as_array().unwrap() {
        let st = c["status"].as_str().unwrap();
        assert!(
            matches!(st, "pass" | "warn" | "fail" | "info" | "ok"),
            "unexpected status {st}"
        );
    }
}

#[test]
fn doctor_fails_without_installed_suite() {
    let dir = tempdir().unwrap();
    // Empty prefix: no suite manifest → must not claim healthy install.
    let out = Command::new(moraine_bin())
        .args(["doctor", "--json"])
        .env("MORAINE_PREFIX", dir.path())
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin",
                moraine_bin().parent().unwrap().display()
            ),
        )
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "doctor must exit nonzero without suite"
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], false);
    let checks = v["checks"].as_array().unwrap();
    assert!(
        checks.iter().any(|c| {
            c["id"] == "suite.manifest" && c["status"] == "fail"
                || c["id"] == "suite.installed" && c["status"] == "fail"
        }),
        "expected suite fail checks: {v}"
    );
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
fn setup_codex_requires_initialized_project() {
    let dir = tempdir().unwrap();
    let out = run(&[
        "setup",
        "codex",
        "--project",
        dir.path().to_str().unwrap(),
        "--json",
    ]);
    assert!(
        !out.status.success(),
        "must fail without project init: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn setup_codex_merges_hooks_and_preserves_user() {
    let dir = tempdir().unwrap();
    let p = dir.path().to_str().unwrap();
    let init = run(&["project", "init", p, "--json"]);
    assert!(init.status.success(), "{}", out_both(&init));

    // Pre-seed unrelated hooks + config
    let codex = dir.path().join(".codex");
    fs::create_dir_all(&codex).unwrap();
    fs::write(
        codex.join("config.toml"),
        "model = \"gpt-test\"\n\n[mcp_servers.other]\ncommand = \"echo\"\n",
    )
    .unwrap();
    fs::write(
        codex.join("hooks.json"),
        r#"{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "echo user-hook" }] }
    ]
  }
}"#,
    )
    .unwrap();

    let out = run(&["setup", "codex", "--project", p, "--json"]);
    assert!(out.status.success(), "{}", out_both(&out));

    let cfg = fs::read_to_string(codex.join("config.toml")).unwrap();
    assert!(cfg.contains("model = \"gpt-test\""));
    assert!(cfg.contains("[mcp_servers.other]"));
    assert!(cfg.contains("[mcp_servers.moraine]"));
    assert!(cfg.contains("# --- Moraine (managed) ---"));

    let hooks: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
    let ups = hooks["hooks"]["UserPromptSubmit"].as_array().unwrap();
    assert!(
        ups.iter().any(|g| g["hooks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["command"] == "echo user-hook")),
        "user hook must survive merge: {hooks}"
    );
    assert!(
        ups.iter().any(|g| g["hooks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["command"].as_str().unwrap_or("").contains("hook-codex"))),
        "managed hook must be present: {hooks}"
    );

    // Idempotent second run
    let out2 = run(&["setup", "codex", "--project", p, "--json"]);
    assert!(out2.status.success());
    let hooks2: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
    let managed_count = hooks2["hooks"]["UserPromptSubmit"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| {
            g["hooks"].as_array().unwrap().iter().any(|h| {
                h.get("moraine-managed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || h["command"].as_str().unwrap_or("").contains("hook-codex")
            })
        })
        .count();
    assert_eq!(managed_count, 1, "must not duplicate managed handlers");

    // Remove managed only
    let rm = run(&["setup", "codex", "--project", p, "--remove", "--json"]);
    assert!(
        rm.status.success(),
        "{}",
        String::from_utf8_lossy(&rm.stdout)
    );
    let cfg_after = fs::read_to_string(codex.join("config.toml")).unwrap();
    assert!(cfg_after.contains("[mcp_servers.other]"));
    assert!(!cfg_after.contains("[mcp_servers.moraine]"));
    let hooks_after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
    let ups_after = hooks_after["hooks"]["UserPromptSubmit"].as_array().unwrap();
    assert!(ups_after.iter().any(|g| g["hooks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["command"] == "echo user-hook")));
    assert!(!serde_json::to_string(&hooks_after)
        .unwrap()
        .contains("hook-codex"));
}

#[test]
fn setup_bare_json() {
    let out = run(&["setup", "--json"]);
    // may warn if suite not installed; still returns JSON
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!(
            "expected json stdout: {}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    assert!(v.get("cli").is_some() || v.get("next").is_some());
}

#[test]
fn setup_codex_refuses_malformed_hooks_json() {
    let dir = tempdir().unwrap();
    let p = dir.path().to_str().unwrap();
    assert!(run(&["project", "init", p, "--json"]).status.success());
    let codex = dir.path().join(".codex");
    fs::create_dir_all(&codex).unwrap();
    fs::write(codex.join("hooks.json"), "NOT JSON {{{").unwrap();
    let before = fs::read_to_string(codex.join("hooks.json")).unwrap();
    let out = run(&["setup", "codex", "--project", p, "--json"]);
    assert!(
        !out.status.success(),
        "must refuse malformed hooks: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = fs::read_to_string(codex.join("hooks.json")).unwrap();
    assert_eq!(before, after, "must not wipe malformed hooks.json");
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        err.contains("malformed") || err.contains("refusing"),
        "actionable error: {err}"
    );
}

fn out_both(o: &std::process::Output) -> String {
    format!(
        "stdout={} stderr={}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}
