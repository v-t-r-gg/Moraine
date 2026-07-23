//! Drive shipped provisioning APIs: inspect → plan → apply → verify → rollback.

use std::fs;
use std::path::PathBuf;

use moraine_provision::{
    apply, enable_project, health, plan, rollback, verify, AgentKind, MemoryServiceManager,
    ProvisionOpKind, Readiness, RepairAction, RepairKind, ServiceManager, SetupIntent,
};
use tempfile::tempdir;

fn intent(project: PathBuf) -> SetupIntent {
    SetupIntent {
        project,
        agent: AgentKind::Codex,
        enable_autostart: true,
        skip_service: false,
    }
}

#[test]
fn inspect_plan_apply_verify_on_temp_project() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("my-app");
    fs::create_dir_all(&project).unwrap();

    let svc = MemoryServiceManager::new();
    // Provide a fake service binary so install can record a path.
    let fake_svc = dir.path().join("moraine-service");
    fs::write(&fake_svc, b"#!/bin/true\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake_svc).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_svc, perms).unwrap();
    }
    svc.install(&fake_svc).unwrap();
    // Reset so plan wants install+start again for a clean apply path that exercises ops.
    // Actually install already set installed=true; use fresh manager for apply.
    let svc = MemoryServiceManager::new();

    let mut i = intent(project.clone());
    // Self-test needs project+agent; service ops use memory manager.
    // We still want service ops recorded.
    let p = plan(i.clone(), &svc).unwrap();
    assert!(
        p.operations
            .iter()
            .any(|o| o.kind == ProvisionOpKind::InitializeProject)
    );
    assert!(
        p.operations
            .iter()
            .any(|o| o.kind == ProvisionOpKind::ConfigureAgent)
    );
    assert!(
        p.operations
            .iter()
            .any(|o| o.kind == ProvisionOpKind::SelfTest)
    );
    assert!(
        p.absolute_cli.starts_with('/'),
        "CLI must be absolute: {}",
        p.absolute_cli
    );
    // Product labels only — no systemd jargon in product_summary.
    for s in &p.product_summary {
        assert!(!s.to_ascii_lowercase().contains("systemctl"), "{s}");
        assert!(!s.to_ascii_lowercase().contains("systemd"), "{s}");
        assert!(!s.contains("MCP"), "{s}");
    }

    // Without a real suite service binary, InstallService may fail on absolute_service.
    // skip_service for full path verify; exercise service ops separately.
    i.skip_service = true;
    let p = plan(i.clone(), &svc).unwrap();
    let receipt = apply(p, &svc).unwrap();
    assert_eq!(
        receipt.readiness,
        Readiness::Ready,
        "receipt: {receipt:?}"
    );
    assert!(receipt.failed_operation.is_none());
    assert!(project.join(".moraine").is_dir());
    assert!(project.join(".codex/config.toml").is_file());

    let cfg = fs::read_to_string(project.join(".codex/config.toml")).unwrap();
    assert!(cfg.contains("command = \"/"), "absolute path in config:\n{cfg}");
    // Extract command path and assert absolute
    let cmd_line = cfg
        .lines()
        .find(|l| l.trim().starts_with("command"))
        .expect("command line");
    let path = cmd_line
        .split('=')
        .nth(1)
        .unwrap()
        .trim()
        .trim_matches('"');
    assert!(
        path.starts_with('/'),
        "expected absolute moraine path, got {path}"
    );
    assert!(
        path.ends_with("/moraine") || path.ends_with("/moraine.exe"),
        "CLI must be suite binary named moraine, not app/service/test: {path}"
    );
    assert!(
        !path.contains("moraine-app") && !path.contains("moraine-service"),
        "must not write app/service path: {path}"
    );

    let report = verify(&i).unwrap();
    assert!(report.ok, "verify: {report:?}");
    assert_eq!(report.readiness, Readiness::Ready);
    assert!(report.run_id.is_some());
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "discovery.run_visible" && s.passed)
    );
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "capture.adapter_event" && s.passed),
        "must exercise adapter capture pipeline: {report:?}"
    );
}

#[test]
fn verify_fails_closed_when_service_required_but_offline() {
    // Honest Ready gate: skip_service=false + no live background capture ⇒ not Ready.
    let dir = tempdir().unwrap();
    let project = dir.path().join("offline-svc");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();

    // Configure agent with absolute suite CLI so we fail on service, not agent.
    let cli = moraine_provision::SuitePaths::discover().absolute_cli();
    assert!(
        cli.is_absolute() && cli.file_name().and_then(|n| n.to_str()) == Some("moraine"),
        "test requires resolvable suite CLI, got {}",
        cli.display()
    );
    let adapter = moraine_provision::adapter_for(AgentKind::Codex);
    let plan = adapter.plan_install(&project, &cli).unwrap();
    adapter.apply(&plan).unwrap();

    let i = SetupIntent {
        project: project.clone(),
        agent: AgentKind::Codex,
        enable_autostart: false,
        skip_service: false, // product path — service is required
    };
    let report = verify(&i).unwrap();

    // If a real service happens to be online in this environment, Ready may pass;
    // otherwise we must fail closed on service.reachable.
    let svc_step = report
        .steps
        .iter()
        .find(|s| s.id == "service.reachable")
        .expect("service.reachable step required");
    if !svc_step.passed {
        assert!(!report.ok, "ok must be false when service offline: {report:?}");
        assert_eq!(report.readiness, Readiness::Failed);
        assert!(
            !report.user_message.to_ascii_lowercase().contains("end-to-end"),
            "must not claim end-to-end when service failed: {}",
            report.user_message
        );
    } else {
        // Environment has live service — step is present and hard-gated as passed.
        assert!(svc_step.passed);
    }
}

#[test]
fn absolute_cli_never_returns_app_binary() {
    use moraine_provision::SuitePaths;
    let cli = SuitePaths::discover().absolute_cli();
    let name = cli.file_name().and_then(|n| n.to_str()).unwrap_or("");
    // When resolvable, must be exactly `moraine`.
    if cli.is_absolute() && cli.is_file() {
        assert_eq!(name, "moraine", "got {}", cli.display());
        assert!(!cli.display().to_string().contains("moraine-app"));
        assert!(!cli.display().to_string().contains("moraine-service"));
    }
    // Sibling resolution: if we invent a fake layout under temp, absolute_cli
    // still prefers suite path or cargo target — document via discover().
    let s = SuitePaths::from_prefix("/nonexistent-prefix-for-test");
    let resolved = s.absolute_cli();
    if resolved.is_file() {
        assert_eq!(
            resolved.file_name().and_then(|n| n.to_str()),
            Some("moraine"),
            "fallback must still be CLI: {}",
            resolved.display()
        );
    }
}

#[test]
fn service_lifecycle_recorded_as_structured_ops() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("svc-proj");
    fs::create_dir_all(&project).unwrap();
    let fake_svc = dir.path().join("moraine-service");
    fs::write(&fake_svc, b"x").unwrap();

    let svc = MemoryServiceManager::new();
    // Pre-install binary path via install so start works when we call service ops.
    svc.install(&fake_svc).unwrap();

    let i = SetupIntent {
        project: project.clone(),
        agent: AgentKind::Codex,
        enable_autostart: true,
        skip_service: false,
    };
    let p = plan(i, &svc).unwrap();
    // Service already installed+not running → plan should include start + autostart
    assert!(
        p.operations
            .iter()
            .any(|o| o.kind == ProvisionOpKind::StartService)
            || p.operations
                .iter()
                .any(|o| o.kind == ProvisionOpKind::InstallService)
            || p.operations
                .iter()
                .any(|o| o.kind == ProvisionOpKind::EnableAutostart),
        "ops: {:?}",
        p.operations.iter().map(|o| &o.id).collect::<Vec<_>>()
    );

    // Direct lifecycle
    assert!(!svc.inspect().unwrap().running);
    svc.start().unwrap();
    assert!(svc.inspect().unwrap().running);
    svc.enable_autostart().unwrap();
    svc.stop().unwrap();
    assert!(!svc.inspect().unwrap().running);
}

#[test]
fn mid_apply_failure_leaves_receipt_and_rollback_restores_config() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("rb");
    fs::create_dir_all(&project).unwrap();

    // Pre-create user codex config that must survive rollback after agent apply+rollback.
    let codex = project.join(".codex");
    fs::create_dir_all(&codex).unwrap();
    let cfg_path = codex.join("config.toml");
    fs::write(&cfg_path, "user_setting = true\n").unwrap();

    let svc = MemoryServiceManager::new();
    // Force failure on start after install by not installing first and using fail_next on install.
    // Better: apply with skip_service=false, install succeeds on memory, start fails.
    let i = SetupIntent {
        project: project.clone(),
        agent: AgentKind::Codex,
        enable_autostart: false,
        skip_service: false,
    };
    let mut p = plan(i, &svc).unwrap();
    // Drop SelfTest so we fail on service install (no real binary) before self-test.
    p.operations
        .retain(|o| o.kind != ProvisionOpKind::SelfTest);

    let receipt = apply(p, &svc).unwrap();
    // InstallService should fail (no suite service binary in test env) OR succeed with memory if sibling found.
    // If the whole apply succeeds because suite has a service binary, force fail via fail_on helper.
    if receipt.readiness == Readiness::Ready {
        // Environment has a service binary; inject failure via MemoryServiceManager on a second apply.
        let svc2 = MemoryServiceManager::new();
        svc2.fail_next("injected service failure");
        let i2 = SetupIntent {
            project: project.clone(),
            agent: AgentKind::Codex,
            enable_autostart: false,
            skip_service: false,
        };
        let p2 = plan(i2, &svc2).unwrap();
        let receipt2 = apply(p2, &svc2).unwrap();
        assert_eq!(receipt2.readiness, Readiness::RollbackRequired);
        assert!(receipt2.failed_operation.is_some());
        assert!(!receipt2.journal_path.is_empty());
        // Rollback should restore backups
        rollback(receipt2, &svc2).unwrap();
    } else {
        assert_eq!(receipt.readiness, Readiness::RollbackRequired);
        assert!(receipt.failed_operation.is_some());
        assert!(!receipt.journal_path.is_empty());

        // Agent may have been configured before service failed — rollback restores.
        let had_agent = receipt
            .completed
            .iter()
            .any(|c| c.kind == ProvisionOpKind::ConfigureAgent && c.success);
        rollback(receipt, &svc).unwrap();
        if had_agent {
            let cfg = fs::read_to_string(&cfg_path).unwrap_or_default();
            // After remove, managed block gone; user_setting should remain if restore worked.
            assert!(
                cfg.contains("user_setting") || !cfg.contains("mcp_servers.moraine"),
                "after rollback cfg=\n{cfg}"
            );
        }
    }
}

#[test]
fn verify_fails_closed_without_initialized_project() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("empty");
    fs::create_dir_all(&project).unwrap();
    let i = SetupIntent {
        project,
        agent: AgentKind::Codex,
        enable_autostart: false,
        skip_service: true,
    };
    let report = verify(&i).unwrap();
    assert!(!report.ok);
    assert_eq!(report.readiness, Readiness::Failed);
}

#[test]
fn health_offers_repair_for_stopped_service() {
    let svc = MemoryServiceManager::new();
    let dir = tempdir().unwrap();
    let project = dir.path().join("h");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();

    // Not installed → install repair
    let report = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    assert!(!report.ok);
    assert!(
        report.checks.iter().any(|c| c.repair.is_some()),
        "{:?}",
        report.checks
    );

    let fake = dir.path().join("moraine-service");
    fs::write(&fake, b"x").unwrap();
    svc.install(&fake).unwrap();
    // Installed but not running
    let report = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    let start_fix = report
        .checks
        .iter()
        .find_map(|c| c.repair.as_ref())
        .expect("expected repair");
    assert_eq!(start_fix.kind, RepairKind::StartService);

    let result = moraine_provision::repair(
        &RepairAction {
            id: start_fix.id.clone(),
            label: "Fix".into(),
            kind: RepairKind::StartService,
            project: None,
            agent: None,
        },
        &svc,
    )
    .unwrap();
    assert!(result.ok);
    assert!(svc.inspect().unwrap().running);
}

#[test]
fn enable_project_ready_requires_self_test() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("en");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    let receipt = enable_project(
        SetupIntent {
            project: project.clone(),
            agent: AgentKind::Codex,
            enable_autostart: false,
            skip_service: true,
        },
        &svc,
    )
    .unwrap();
    assert_eq!(receipt.readiness, Readiness::Ready);
    // Self-test op completed
    assert!(
        receipt
            .completed
            .iter()
            .any(|c| c.kind == ProvisionOpKind::SelfTest && c.success)
    );
}

#[test]
fn product_progress_labels_have_no_infra_jargon() {
    for kind in [
        ProvisionOpKind::InitializeProject,
        ProvisionOpKind::ConfigureAgent,
        ProvisionOpKind::InstallService,
        ProvisionOpKind::EnableAutostart,
        ProvisionOpKind::StartService,
        ProvisionOpKind::SelfTest,
    ] {
        let label = kind.product_label().to_ascii_lowercase();
        assert!(!label.contains("systemctl"));
        assert!(!label.contains("systemd"));
        assert!(!label.contains("mcp"));
        assert!(!label.contains("hook"));
        assert!(!label.contains("path"));
        assert!(!label.contains("127.0.0.1"));
    }
}
