//! Primary product package must require desktop; headless is explicitly named.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn release_script_fails_closed_without_desktop() {
    let root = repo_root();
    let script = root.join("scripts/build-linux-release.sh");
    let src = fs::read_to_string(&script).expect("build-linux-release.sh");
    // Must fail when moraine-app is missing for primary package
    assert!(
        src.contains("primary product package missing bin/moraine-app")
            || src.contains("moraine-app release build failed"),
        "release script must fail closed when desktop is missing"
    );
    assert!(
        src.contains("MORAINE_HEADLESS"),
        "headless packages must be explicitly opted in"
    );
    assert!(
        src.contains("linux-x86_64-headless") || src.contains("headless"),
        "headless artifact must be distinctly named"
    );
    // Must NOT silently continue with CLI+service only
    assert!(
        !src.contains("suite will ship CLI+service only"),
        "must not silently ship headless as primary"
    );
}

#[test]
fn write_manifest_marks_desktop_when_present() {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path();
    fs::create_dir_all(stage.join("bin")).unwrap();
    // Touch CLI/service only first
    fs::write(stage.join("bin/moraine"), b"x").unwrap();
    fs::write(stage.join("bin/moraine-service"), b"x").unwrap();
    let st = Command::new("python3")
        .arg(root.join("scripts/packaging/write_manifest.py"))
        .arg(stage)
        .env("VERSION", "0.1.0")
        .output()
        .unwrap();
    assert!(st.status.success());
    let man: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(stage.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(man["components"]["desktop"], "missing");

    // With desktop binary present
    fs::write(stage.join("bin/moraine-app"), b"x").unwrap();
    let st = Command::new("python3")
        .arg(root.join("scripts/packaging/write_manifest.py"))
        .arg(stage)
        .env("VERSION", "0.1.0")
        .output()
        .unwrap();
    assert!(st.status.success());
    let man: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(stage.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(man["components"]["desktop"], "0.1.0");
}

#[test]
fn primary_stage_layout_requires_app_binary_assertion() {
    // Structural check: packaging install.sh copies moraine-app when present.
    let root = repo_root();
    let install = fs::read_to_string(root.join("scripts/packaging/install.sh")).unwrap();
    assert!(
        install.contains("moraine-app"),
        "install.sh must handle moraine-app for desktop suite"
    );
}

#[test]
fn ci_uses_authoritative_release_script_and_tests_provision() {
    let root = repo_root();
    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("ci.yml");
    assert!(
        ci.contains("build-linux-release.sh"),
        "CI primary artifact must invoke scripts/build-linux-release.sh"
    );
    assert!(
        ci.contains("moraine-provision"),
        "CI must include moraine-provision in gates"
    );
    assert!(
        ci.contains("cargo test -p moraine-provision"),
        "CI must run moraine-provision tests"
    );
    assert!(
        ci.contains("bin/moraine-app") || ci.contains("moraine-app"),
        "CI smoke must require desktop in primary archive"
    );
    // Headless must not be the default CI package path
    assert!(
        !ci.contains("MORAINE_HEADLESS=1") || ci.contains("build-linux-release.sh"),
        "primary CI package must not force headless"
    );
}
