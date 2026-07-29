//! Temp-prefix installer/uninstaller smoke (drives shipped packaging scripts).

#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tree_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(current) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, out);
            } else if path.is_file() {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    if root.is_dir() {
        visit(root, root, &mut out);
    }
    out
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
    let cache = tempdir().unwrap();
    let runtime = tempdir().unwrap();
    let project = tempdir().unwrap();
    fs::create_dir_all(project.path().join(".moraine")).unwrap();
    fs::write(project.path().join(".moraine/keep.txt"), "ledger").unwrap();

    let install = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .env("XDG_CACHE_HOME", cache.path())
        .env("XDG_RUNTIME_DIR", runtime.path())
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
    assert!(unit_txt.contains("--http 127.0.0.1:33111"));
    assert!(unit_txt.contains(&format!(
        "--unix-socket {}",
        runtime.path().join("moraine-service.sock").display()
    )));
    assert!(unit_txt.contains(&format!(
        "--spool-dir {}",
        cache.path().join("moraine-service/spool").display()
    )));
    assert!(!unit_txt.contains(".cargo/bin"));

    // same-version reinstall
    let re = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .env("XDG_CACHE_HOME", cache.path())
        .env("XDG_RUNTIME_DIR", runtime.path())
        .output()
        .unwrap();
    assert!(re.status.success());

    let un = Command::new(bundle.path().join("uninstall.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .env("XDG_CACHE_HOME", cache.path())
        .env("XDG_RUNTIME_DIR", runtime.path())
        .output()
        .unwrap();
    assert!(un.status.success());
    assert!(
        !String::from_utf8_lossy(&un.stdout).contains("legacy runtime-registration"),
        "normal uninstall unexpectedly used legacy fallback"
    );
    assert!(!cli.exists());
    assert!(!unit.exists());
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

#[test]
fn registration_failure_removes_fresh_partial_install() {
    let bundle = tempdir().unwrap();
    stage_min_bundle(bundle.path());
    let parent = tempdir().unwrap();
    let prefix = parent.path().join("fresh-prefix");
    let blocked_config = parent.path().join("config-is-a-file");
    fs::write(&blocked_config, b"block registration directory").unwrap();
    let cache = tempdir().unwrap();
    let runtime = tempdir().unwrap();

    let output = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(&prefix)
        .env("XDG_CONFIG_HOME", &blocked_config)
        .env("XDG_CACHE_HOME", cache.path())
        .env("XDG_RUNTIME_DIR", runtime.path())
        .env("HOME", parent.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("background runtime registration"));
    assert!(
        !prefix.exists(),
        "partial fresh prefix retained: {prefix:?}"
    );
    assert_eq!(
        fs::read(&blocked_config).unwrap(),
        b"block registration directory"
    );
}

#[test]
fn registration_failure_restores_existing_installation_exactly() {
    let bundle = tempdir().unwrap();
    stage_min_bundle(bundle.path());
    let prefix = tempdir().unwrap();
    let parent = tempdir().unwrap();
    let blocked_config = parent.path().join("config-is-a-file");
    fs::write(&blocked_config, b"block registration directory").unwrap();
    let cache = tempdir().unwrap();
    let runtime = tempdir().unwrap();

    let seeded = [
        ("bin/moraine", b"old cli".as_slice()),
        ("libexec/moraine/moraine-service", b"old service".as_slice()),
        ("libexec/moraine/keep", b"keep libexec".as_slice()),
        ("lib/moraine/keep", b"keep desktop dir".as_slice()),
        ("share/moraine/keep", b"keep share".as_slice()),
        (
            "share/applications/app.moraine.desktop",
            b"old desktop entry".as_slice(),
        ),
        (
            "share/icons/hicolor/128x128/apps/app.moraine.png",
            b"old icon".as_slice(),
        ),
    ];
    for (relative, bytes) in seeded {
        let path = prefix.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let before = tree_snapshot(prefix.path());

    let output = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", &blocked_config)
        .env("XDG_CACHE_HOME", cache.path())
        .env("XDG_RUNTIME_DIR", runtime.path())
        .env("HOME", parent.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("background runtime registration"));
    assert_eq!(tree_snapshot(prefix.path()), before);
    assert_eq!(
        fs::read(&blocked_config).unwrap(),
        b"block registration directory"
    );
}

#[test]
fn registration_failure_restores_prior_unit_exactly() {
    let bundle = tempdir().unwrap();
    stage_min_bundle(bundle.path());
    let prefix = tempdir().unwrap();
    let config = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let runtime = tempdir().unwrap();
    let unit = config.path().join("systemd/user/moraine-service.service");
    fs::create_dir_all(unit.parent().unwrap()).unwrap();
    let prior = b"[Service]\nExecStart=/prior/moraine-service\n";
    fs::write(&unit, prior).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&unit, fs::Permissions::from_mode(0o444)).unwrap();
    }

    let output = Command::new(bundle.path().join("install.sh"))
        .arg("--prefix")
        .arg(prefix.path())
        .env("XDG_CONFIG_HOME", config.path())
        .env("XDG_CACHE_HOME", cache.path())
        .env("XDG_RUNTIME_DIR", runtime.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(fs::read(&unit).unwrap(), prior);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&unit).unwrap().permissions().mode() & 0o777,
            0o444
        );
    }
}
