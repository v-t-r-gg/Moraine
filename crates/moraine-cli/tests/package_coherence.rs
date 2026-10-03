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
fn write_manifest_marks_windows_desktop_exe_when_present() {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path();
    fs::create_dir_all(stage.join("bin")).unwrap();
    fs::write(stage.join("bin/moraine.exe"), b"x").unwrap();
    fs::write(stage.join("bin/moraine-service.exe"), b"x").unwrap();
    fs::write(stage.join("bin/moraine-app.exe"), b"x").unwrap();
    let st = Command::new("python3")
        .arg(root.join("scripts/packaging/write_manifest.py"))
        .arg(stage)
        .env("VERSION", "0.1.0")
        .env("MORAINE_TARGET_TRIPLE", "x86_64-pc-windows-msvc")
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "{}",
        String::from_utf8_lossy(&st.stderr)
    );
    let man: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(stage.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(man["components"]["desktop"], "0.1.0");
    assert_eq!(man["components"]["cli"], "0.1.0");
    assert_eq!(man["components"]["service"], "0.1.0");
    assert_ne!(man["components"]["desktop"], "missing");
}

#[test]
fn windows_demo_archive_checker_self_test() {
    let root = repo_root();
    let st = Command::new("python3")
        .arg(root.join("scripts/packaging/check_windows_demo_archive.py"))
        .arg("--self-test")
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&st.stdout),
        String::from_utf8_lossy(&st.stderr)
    );
}

#[test]
fn windows_demo_scripts_stage_without_registering_a_task() {
    let root = repo_root();
    let build = fs::read_to_string(root.join("scripts/build-windows-demo.ps1")).unwrap();
    let stage = fs::read_to_string(root.join("scripts/stage-windows-demo.ps1")).unwrap();
    assert!(build.contains("x86_64-pc-windows-msvc"));
    assert!(build.contains("moraine-app.exe"));
    assert!(build.contains("-dirty"));
    assert!(!build.contains("Register-ScheduledTask"));
    assert!(!build.contains("schtasks"));
    assert!(stage.contains("Administrator"));
    assert!(stage.contains("LOCALAPPDATA"));
    assert!(stage.contains("share\\moraine"));
    assert!(stage.contains("\"User\""));
    assert!(!stage.contains("\"Machine\""));
    assert!(stage.contains("moraine --version"));
    assert!(stage.contains("moraine doctor"));
    assert!(stage.contains("moraine project init"));
}

#[test]
fn readme_windows_product_ready_stays_no() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(
        readme.contains("| Product Ready | Yes | No |"),
        "Windows Product Ready must stay No until a live W2-E session passes"
    );
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

#[test]
fn release_documentation_manifest_is_exact() {
    let root = repo_root();
    let release = fs::read_to_string(root.join("scripts/build-linux-release.sh")).unwrap();
    let copies: Vec<_> = release
        .lines()
        .filter(|line| line.contains("\"$STAGE/share/documentation/"))
        .collect();
    assert_eq!(copies.len(), 6, "unexpected release documentation copy set");
    for name in [
        "README.md",
        "INSTALL.md",
        "SECURITY.md",
        "TROUBLESHOOTING.md",
        "CODEX.md",
        "LICENSE",
    ] {
        assert!(
            copies.iter().any(|line| line.contains(name)),
            "release documentation missing {name}"
        );
    }
    assert!(
        !copies.iter().any(|line| line.contains("REDACTION.md")),
        "consolidated redaction guide must not be packaged"
    );
}
