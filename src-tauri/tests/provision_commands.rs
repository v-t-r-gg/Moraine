//! Structural + type-level proof that desktop provision commands call the shared crate.

use moraine_provision::{
    apply, health, plan, verify, AgentKind, ApplyOutcome, MemoryServiceManager, ProvisionOpKind,
    Readiness, SetupIntent,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn shared_provision_apis_usable_from_desktop_crate() {
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
    let p = plan(intent.clone(), &svc).unwrap();
    assert!(p.operations.iter().any(|o| o.kind == ProvisionOpKind::SelfTest));
    assert!(!p.plan_id.is_nil());
    let outcome = apply(p, &svc).unwrap();
    assert!(matches!(outcome, ApplyOutcome::DirectVerified { .. }));
    assert_eq!(outcome.receipt().readiness, Readiness::DirectVerified);
    let report = verify(&intent).unwrap();
    assert!(report.ok);
    assert_eq!(report.readiness, Readiness::DirectVerified);
    let h = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    assert!(!h.checks.is_empty());
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
        assert!(lib.contains(cmd), "lib.rs must register Tauri command {cmd}");
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
