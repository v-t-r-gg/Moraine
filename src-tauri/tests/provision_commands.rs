//! Structural + type-level proof that desktop provision commands call the shared crate.
//! Hermetic: injects absolute fake CLI via MORAINE_CLI (no workspace build required).

use moraine_provision::{
    apply, health, plan, verify, AgentKind, ApplyOutcome, MemoryServiceManager, ProvisionOpKind,
    Readiness, SetupIntent,
};
use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;

/// Serialize env mutation across tests in this binary.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_fake_cli<R>(f: impl FnOnce(std::path::PathBuf) -> R) -> R {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let cli = dir.path().join("moraine");
    fs::write(&cli, b"#!/bin/true\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&cli).unwrap().permissions();
        p.set_mode(0o755);
        fs::set_permissions(&cli, p).unwrap();
    }
    // Clear ambient suite discovery that would prefer missing installed paths.
    let prev_cli = std::env::var("MORAINE_CLI").ok();
    let prev_prefix = std::env::var("MORAINE_PREFIX").ok();
    std::env::set_var("MORAINE_CLI", &cli);
    // Use a nonexistent prefix so SuitePaths::cli is not a real file.
    std::env::set_var("MORAINE_PREFIX", dir.path().join("no-suite"));
    let out = f(cli.clone());
    match prev_cli {
        Some(v) => std::env::set_var("MORAINE_CLI", v),
        None => std::env::remove_var("MORAINE_CLI"),
    }
    match prev_prefix {
        Some(v) => std::env::set_var("MORAINE_PREFIX", v),
        None => std::env::remove_var("MORAINE_PREFIX"),
    }
    out
}

#[test]
fn shared_provision_apis_usable_from_desktop_crate() {
    with_fake_cli(|cli| {
        assert!(cli.is_absolute() || cli.exists());
        let suite_cli = moraine_provision::SuitePaths::discover().absolute_cli();
        assert!(
            suite_cli.is_absolute() && suite_cli.is_file(),
            "injected CLI must resolve absolutely: {}",
            suite_cli.display()
        );

        let dir = tempdir().unwrap();
        let project = dir.path().join("desk");
        fs::create_dir_all(&project).unwrap();
        let svc = MemoryServiceManager::new();
        let intent = SetupIntent {
            project: project.clone(),
            agent: AgentKind::Codex,
            enable_autostart: false,
            skip_service: true,
        };
        let p = plan(intent.clone(), &svc).expect("plan must succeed with injected CLI");
        assert!(p
            .operations
            .iter()
            .any(|o| o.kind == ProvisionOpKind::SelfTest));
        assert!(!p.plan_id.is_nil());
        assert!(p.absolute_cli.starts_with('/'));
        let outcome = apply(p, &svc).expect("apply");
        assert!(matches!(outcome, ApplyOutcome::DirectVerified { .. }));
        assert_eq!(outcome.receipt().readiness, Readiness::DirectVerified);
        let report = verify(&intent).expect("verify");
        assert!(report.ok);
        assert_eq!(report.readiness, Readiness::DirectVerified);
        let h = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
        assert!(!h.checks.is_empty());
    });
}

#[test]
fn provision_command_source_registers_shared_handlers() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    for cmd in [
        "provision_inspect",
        "provision_plan",
        "provision_apply",
        "provision_apply_plan",
        "provision_verify",
        "provision_health",
        "provision_repair",
        "provision_enable",
    ] {
        assert!(
            lib.contains(cmd),
            "lib.rs must register Tauri command {cmd}"
        );
    }
    let prov = fs::read_to_string(root.join("src/commands/provision.rs")).unwrap();
    assert!(
        prov.contains("moraine_provision"),
        "provision commands must call shared crate, not CLI scrape"
    );
    assert!(
        prov.contains("provision_apply_plan"),
        "must expose apply of approved plan"
    );
    assert!(
        !prov.contains("Command::new(\"moraine\")"),
        "must not shell out to CLI"
    );
}
