//! Temp-prefix installer/uninstaller smoke (drives shipped packaging scripts).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn stage_min_bundle(stage: &std::path::Path) {
    let root = repo_root();
    let moraine = root.join("target/debug/moraine");
    let service = root.join("target/debug/moraine-service");
    assert!(moraine.is_file(), "build moraine first");
    // service may only exist after build -p moraine-service
    if !service.is_file() {
        let st = Command::new("cargo")
            .args(["build", "-p", "moraine-service", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(st.success());
    }
    fs::create_dir_all(stage.join("bin")).unwrap();
    fs::create_dir_all(stage.join("systemd")).unwrap();
    fs::create_dir_all(stage.join("share/documentation")).unwrap();
    fs::copy(&moraine, stage.join("bin/moraine")).unwrap();
    fs::copy(
        root.join("target/debug/moraine-service"),
        stage.join("bin/moraine-service"),
    )
    .unwrap();
    // executable bit
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["moraine", "moraine-service"] {
            let p = stage.join("bin").join(name);
            let mut perms = fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&p, perms).unwrap();
        }
    }
    fs::copy(
        root.join("crates/moraine-service/systemd/moraine-service.service.in"),
        stage.join("systemd/moraine-service.service.in"),
    )
    .unwrap();
    fs::copy(
        root.join("scripts/packaging/install.sh"),
        stage.join("install.sh"),
    )
    .unwrap();
    fs::copy(
        root.join("scripts/packaging/uninstall.sh"),
        stage.join("uninstall.sh"),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["install.sh", "uninstall.sh"] {
            let p = stage.join(name);
            let mut perms = fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&p, perms).unwrap();
        }
    }
    let version = env!("CARGO_PKG_VERSION");
    let manifest = serde_json::json!({
        "product": "Moraine",
        "version": version,
        "gitCommit": "test",
        "target": "x86_64-unknown-linux-gnu",
        "profile": "debug",
        "schema": { "minimumReadable": 3, "maximumReadable": 6, "currentWritable": 6 },
        "serviceProtocolVersion": 1,
        "mcpImplementationVersion": 1,
        "components": { "cli": version, "service": version, "desktop": "missing" }
    });
    fs::write(
        stage.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn install_reinstall_uninstall_preserves_project_ledger() {
    let bundle = tempdir().unwrap();
    stage_min_bundle(bundle.path());
    let prefix = tempdir().unwrap();
    let xdg = tempdir().unwrap();
    let project = tempdir().unwrap();
    fs::create_dir_all(project.path().join(".moraine")).unwrap();
    fs::write(project.path().join(".moraine/keep.txt"), "ledger").unwrap();

    let install = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .env("HOME", xdg.path().parent().unwrap()) // not used as prefix
        .output()
        .unwrap();
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    let cli = prefix.path().join("bin/moraine");
    let svc = prefix.path().join("libexec/moraine/moraine-service");
    assert!(cli.is_file());
    assert!(svc.is_file());
    let unit = xdg.path().join("systemd/user/moraine-service.service");
    assert!(unit.is_file());
    let unit_txt = fs::read_to_string(&unit).unwrap();
    assert!(unit_txt.contains("libexec/moraine/moraine-service"));
    assert!(!unit_txt.contains(".cargo/bin"));

    // same-version reinstall
    let re = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .output()
        .unwrap();
    assert!(re.status.success());

    let un = Command::new(bundle.path().join("uninstall.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .output()
        .unwrap();
    assert!(un.status.success());
    assert!(!cli.exists());
    assert!(project.path().join(".moraine/keep.txt").is_file());
}

#[test]
fn install_rejects_incoherent_manifest() {
    let bundle = tempdir().unwrap();
    stage_min_bundle(bundle.path());
    let bad = serde_json::json!({
        "product": "Moraine",
        "version": "0.1.0",
        "gitCommit": "test",
        "target": "x86_64-unknown-linux-gnu",
        "profile": "debug",
        "schema": { "minimumReadable": 3, "maximumReadable": 6, "currentWritable": 6 },
        "serviceProtocolVersion": 1,
        "mcpImplementationVersion": 1,
        "components": { "cli": "0.1.0", "service": "9.9.9", "desktop": "missing" }
    });
    fs::write(
        bundle.path().join("manifest.json"),
        serde_json::to_string_pretty(&bad).unwrap(),
    )
    .unwrap();
    let prefix = tempdir().unwrap();
    let out = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
}
