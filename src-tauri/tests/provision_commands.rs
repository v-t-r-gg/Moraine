//! Structural + type-level proof that desktop provision commands call the shared crate.
//!
//! Full Tauri IPC is not driven here; we assert command modules link and shared APIs work.

use moraine_provision::{
    apply, health, plan, verify, AgentKind, MemoryServiceManager, ProvisionOpKind, Readiness,
    SetupIntent,
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
    let receipt = apply(p, &svc).unwrap();
    assert_eq!(receipt.readiness, Readiness::Ready);
    let report = verify(&intent).unwrap();
    assert!(report.ok);
    let h = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    assert!(!h.checks.is_empty());
}

#[test]
fn provision_command_source_registers_shared_handlers() {
    // Static proof: command module and lib invoke_handler list the provision surface.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    for cmd in [
        "provision_inspect",
        "provision_plan",
        "provision_apply",
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
        !prov.contains("Command::new(\"moraine\")"),
        "must not shell out to CLI"
    );
}
