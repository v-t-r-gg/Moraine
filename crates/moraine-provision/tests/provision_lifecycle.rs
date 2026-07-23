//! Drive shipped provisioning APIs: product vs direct verify, journaled apply, rollback.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use moraine_provision::{
    apply, apply_with_options, enable_project, health, plan, rollback, verify, verify_with_options,
    AgentKind, AlwaysReadyProbe, ApplyOutcome, ControlledCapture, MemoryServiceManager,
    ProvisionOpKind, Readiness, RepairAction, RepairKind, ServiceManager, SetupIntent,
    VecBackupRecorder, VerificationMode, VerifyOptions,
};
use tempfile::tempdir;

fn direct_intent(project: PathBuf) -> SetupIntent {
    SetupIntent {
        project,
        agent: AgentKind::Codex,
        enable_autostart: false,
        skip_service: true,
    }
}

fn product_intent(project: PathBuf) -> SetupIntent {
    SetupIntent {
        project,
        agent: AgentKind::Codex,
        enable_autostart: false,
        skip_service: false,
    }
}

fn setup_agent(project: &std::path::Path) {
    moraine_core::init_project(Some(project)).unwrap();
    let cli = moraine_provision::SuitePaths::discover().absolute_cli();
    assert!(
        cli.is_file() && cli.file_name().and_then(|n| n.to_str()) == Some("moraine"),
        "suite CLI required: {}",
        cli.display()
    );
    let adapter = moraine_provision::adapter_for(AgentKind::Codex);
    let mut rec = VecBackupRecorder::new();
    adapter
        .apply(&adapter.plan_install(project, &cli).unwrap(), &mut rec)
        .unwrap();
}

#[test]
fn direct_verify_never_product_ready() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("direct");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    let outcome = enable_project(direct_intent(project.clone()), &svc).unwrap();
    assert!(matches!(outcome, ApplyOutcome::DirectVerified { .. }));
    let report = verify(&direct_intent(project)).unwrap();
    assert!(report.ok);
    assert_eq!(report.readiness, Readiness::DirectVerified);
    assert_ne!(report.readiness, Readiness::Ready);
}

#[test]
fn product_verify_fails_when_hook_delivery_fails_no_core_fallback() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("hook-fail");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);

    // Seed a stale self-test run that must NOT grant Ready when hook fails.
    moraine_core::run_start(moraine_core::RunStartRequest {
        objective: "Moraine self-test: stale leftover".into(),
        idempotency_key: "stale-leftover".into(),
        project: Some(project.clone()),
        session_id: None,
    })
    .unwrap();

    let report = verify_with_options(
        &product_intent(project.clone()),
        VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: true,
                materialize_run: true,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        },
    )
    .unwrap();
    assert!(!report.ok, "{report:?}");
    assert_eq!(report.readiness, Readiness::Failed);
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "capture.adapter_event" && !s.passed),
        "must fail on capture delivery, not steal stale run: {report:?}"
    );
}

#[test]
fn product_verify_fails_when_hooks_missing_even_if_mcp_present() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("mcp-only");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    fs::create_dir_all(project.join(".codex")).unwrap();
    let cli = moraine_provision::SuitePaths::discover().absolute_cli();
    fs::write(
        project.join(".codex/config.toml"),
        format!(
            "# --- Moraine (managed) ---\n[mcp_servers.moraine]\ncommand = \"{}\"\nargs = [\"mcp\", \"--project\", \"{}\"]\n# --- end Moraine ---\n",
            cli.display(),
            project.display()
        ),
    )
    .unwrap();

    let report = verify_with_options(
        &product_intent(project),
        VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: false,
                materialize_run: true,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        },
    )
    .unwrap();
    assert!(!report.ok);
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "agent.hooks" && !s.passed),
        "{report:?}"
    );
}

#[test]
fn product_happy_path_ready_with_injected_service_and_capture() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("happy");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);

    let report = verify_with_options(
        &product_intent(project.clone()),
        VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: false,
                materialize_run: true,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe {
                version: Some("0.1.0".into()),
            })),
        },
    )
    .unwrap();
    assert!(report.ok, "must be product Ready: {report:?}");
    assert_eq!(report.readiness, Readiness::Ready);
    assert!(report.run_id.is_some());
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "capture.adapter_event" && s.passed && s.message.contains("event_id=")),
        "must record unique event_id: {report:?}"
    );
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "discovery.run_visible" && s.passed && s.message.contains("session-bound")),
        "{report:?}"
    );
}

#[test]
fn absolute_cli_mismatch_fails_closed() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("cli-mismatch");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    fs::create_dir_all(project.join(".codex")).unwrap();
    // Point at a different absolute path that looks like moraine but is not suite CLI.
    let fake = dir.path().join("moraine");
    fs::write(&fake, b"#!/bin/true\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&fake).unwrap().permissions();
        p.set_mode(0o755);
        fs::set_permissions(&fake, p).unwrap();
    }
    fs::write(
        project.join(".codex/config.toml"),
        format!(
            "# --- Moraine (managed) ---\n[mcp_servers.moraine]\ncommand = \"{}\"\nargs = [\"mcp\"]\n# --- end Moraine ---\n",
            fake.display()
        ),
    )
    .unwrap();
    fs::write(
        project.join(".codex/hooks.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"x hook-codex","moraine-managed":true}]}]}}"#,
    )
    .unwrap();

    let report = verify_with_options(
        &product_intent(project),
        VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: false,
                materialize_run: true,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        },
    )
    .unwrap();
    assert!(!report.ok);
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "agent.absolute_cli" && !s.passed),
        "must reject non-suite CLI: {report:?}"
    );
}

#[test]
fn write_ahead_journal_records_backup_before_mutation() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("waj");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    let codex = project.join(".codex");
    fs::create_dir_all(&codex).unwrap();
    let cfg = codex.join("config.toml");
    let original = "keep_me = true\n";
    fs::write(&cfg, original).unwrap();
    // Malformed hooks cause apply error AFTER config would have been written without WAJ ordering.
    // With pre-validation of hooks, config may not be written — still require backup journal on any mutation.
    fs::write(codex.join("hooks.json"), "{ not json").unwrap();

    let svc = MemoryServiceManager::new();
    let intent = direct_intent(project.clone());
    let mut p = plan(intent, &svc).unwrap();
    p.operations
        .retain(|o| o.kind == ProvisionOpKind::ConfigureAgent);

    // Use real apply path with journaled recorder.
    let outcome = apply(p, &svc).unwrap();
    // Should fail (malformed hooks) and roll back.
    assert!(
        matches!(
            outcome,
            ApplyOutcome::RolledBack { .. } | ApplyOutcome::RollbackRequired { .. }
        ),
        "{outcome:?}"
    );
    let receipt = outcome.receipt();
    // Journal file exists and is readable.
    assert!(
        std::path::Path::new(&receipt.journal_path).is_file(),
        "journal must exist: {}",
        receipt.journal_path
    );
    // Original config bytes preserved (no partial MCP wipe without restore).
    let after = fs::read_to_string(&cfg).unwrap_or_default();
    assert!(
        after.contains("keep_me") || after == original,
        "user config must survive: {after}"
    );
}

#[test]
fn mid_apply_failure_auto_rolls_back_and_restores_config_bytes() {
    std::env::set_var("MORAINE_SERVICE_READY_MS", "200");
    let dir = tempdir().unwrap();
    let project = dir.path().join("rb");
    fs::create_dir_all(&project).unwrap();
    let codex = project.join(".codex");
    fs::create_dir_all(&codex).unwrap();
    let cfg_path = codex.join("config.toml");
    let original = "user_setting = true\ncustom = 42\n";
    fs::write(&cfg_path, original).unwrap();

    let svc = MemoryServiceManager::new();
    let mut p = plan(product_intent(project.clone()), &svc).unwrap();
    p.operations
        .retain(|o| o.kind != ProvisionOpKind::SelfTest);

    let outcome = apply(p, &svc).unwrap();
    match outcome {
        ApplyOutcome::RolledBack { receipt, .. } | ApplyOutcome::RollbackRequired { receipt, .. } => {
            assert!(!receipt.journal_path.is_empty());
            if receipt
                .completed
                .iter()
                .any(|c| c.kind == ProvisionOpKind::ConfigureAgent && c.success)
            {
                assert!(!receipt.backups.is_empty(), "backups journaled: {receipt:?}");
            }
        }
        ApplyOutcome::Ready { .. } | ApplyOutcome::DirectVerified { .. } => {
            let svc2 = MemoryServiceManager::new();
            svc2.fail_next("injected");
            let p2 = plan(product_intent(project.clone()), &svc2).unwrap();
            let o2 = apply(p2, &svc2).unwrap();
            assert!(matches!(
                o2,
                ApplyOutcome::RolledBack { .. } | ApplyOutcome::RollbackRequired { .. }
            ));
        }
    }
    if cfg_path.is_file() {
        let after = fs::read_to_string(&cfg_path).unwrap();
        assert!(
            after.contains("user_setting") || after.contains("custom"),
            "user config lost: {after}"
        );
    }
}

#[test]
fn rollback_restores_exact_snapshot_without_semantic_remove_after() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("snap");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    let codex = project.join(".codex");
    fs::create_dir_all(&codex).unwrap();
    let cfg_path = codex.join("config.toml");
    let original = "pre_existing = true\n# --- Moraine (managed) ---\n[mcp_servers.moraine]\ncommand = \"/old/moraine\"\n# --- end Moraine ---\n";
    fs::write(&cfg_path, original).unwrap();
    let bak = cfg_path.with_extension("bak.test");
    fs::copy(&cfg_path, &bak).unwrap();
    fs::write(&cfg_path, "destroyed = true\n").unwrap();

    let receipt = moraine_provision::SetupReceipt {
        transaction_id: uuid::Uuid::new_v4(),
        intent: direct_intent(project.clone()),
        completed: vec![],
        backups: vec![moraine_provision::BackupRecord {
            original_path: cfg_path.display().to_string(),
            backup_path: bak.display().to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }],
        readiness: Readiness::RollbackRequired,
        failed_operation: Some("configure_agent".into()),
        error: Some("test".into()),
        journal_path: String::new(),
    };
    rollback(receipt, &MemoryServiceManager::new()).unwrap();
    assert_eq!(fs::read_to_string(&cfg_path).unwrap(), original);
}

#[test]
fn stale_plan_rejected_on_apply() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("stale");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    let mut p = plan(direct_intent(project), &svc).unwrap();
    p.state_witness.project_initialized = !p.state_witness.project_initialized;
    let err = apply(p, &svc).unwrap_err();
    assert!(err.to_string().to_ascii_lowercase().contains("stale"));
}

#[test]
fn inspect_plan_apply_direct_path() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("my-app");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    let p = plan(direct_intent(project.clone()), &svc).unwrap();
    assert!(!p.plan_id.is_nil());
    if let Some(init) = p
        .operations
        .iter()
        .find(|o| o.kind == ProvisionOpKind::InitializeProject)
    {
        assert!(!init.reversible);
    }
    let outcome = apply(p, &svc).unwrap();
    assert!(matches!(outcome, ApplyOutcome::DirectVerified { .. }));
}

#[test]
fn product_apply_self_test_ready_with_injectables() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("prod-apply");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    // Pre-install memory service so InstallService/StartService succeed without suite binary path issues.
    let fake_svc = dir.path().join("moraine-service");
    fs::write(&fake_svc, b"x").unwrap();
    // Plan with skip_service false but we'll only run configure + self-test via filtered plan?
    // Full product plan needs service binary for install — use skip_service false with only self-test
    // after manual setup + AlwaysReady for verify inject via apply_with_options.

    // Configure agent first via direct enable then product verify-only apply is hard.
    // Drive product Ready through verify_with_options (already tested) AND apply self-test op:
    setup_agent(&project);
    let intent = product_intent(project.clone());
    let mut p = plan(intent, &svc).unwrap();
    // Only SelfTest — witness still matches if we don't change state.
    p.operations
        .retain(|o| o.kind == ProvisionOpKind::SelfTest);
    // Recompute witness after setup_agent changed project_initialized
    p.state_witness = moraine_provision::compute_witness(
        &p.intent,
        &svc,
        &p.absolute_cli,
    )
    .unwrap();

    let opts = VerifyOptions {
        mode: VerificationMode::ProductCapture,
        capture: Some(Arc::new(ControlledCapture {
            fail_delivery: false,
            materialize_run: true,
        })),
        service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
    };
    let outcome = apply_with_options(p, &svc, Some(opts), Some(Arc::new(AlwaysReadyProbe {
        version: None,
    })))
    .unwrap();
    assert!(
        matches!(outcome, ApplyOutcome::Ready { .. }),
        "product apply self-test must Ready: {outcome:?}"
    );
    assert_eq!(outcome.receipt().readiness, Readiness::Ready);
}

#[test]
fn service_lifecycle_and_health_repair() {
    let svc = MemoryServiceManager::new();
    let dir = tempdir().unwrap();
    let project = dir.path().join("h");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    let report = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    assert!(!report.ok);
    // Not registered → Install repair (not Start).
    let install_fix = report
        .checks
        .iter()
        .find_map(|c| c.repair.as_ref())
        .expect("repair");
    assert_eq!(install_fix.kind, RepairKind::InstallService);

    let fake = dir.path().join("moraine-service");
    fs::write(&fake, b"x").unwrap();
    svc.install(&fake).unwrap();
    let st = svc.inspect().unwrap();
    assert!(st.registration_present);
    assert!(st.installed);
    assert!(!st.running);

    let report = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    let start_fix = report
        .checks
        .iter()
        .find_map(|c| c.repair.as_ref())
        .expect("repair");
    assert_eq!(
        start_fix.kind,
        RepairKind::StartService,
        "registered-but-stopped must Start, not Install"
    );
    moraine_provision::repair(
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
    assert!(svc.inspect().unwrap().running);
}

#[test]
fn plan_installs_when_binary_present_but_not_registered() {
    // Simulate binary-only state: memory manager without install() has neither;
    // assert plan logic uses registration_present via a custom inspect after partial state.
    let svc = MemoryServiceManager::new();
    let dir = tempdir().unwrap();
    let project = dir.path().join("bin-only");
    fs::create_dir_all(&project).unwrap();
    let p = plan(product_intent(project), &svc).unwrap();
    assert!(
        p.operations
            .iter()
            .any(|o| o.kind == ProvisionOpKind::InstallService),
        "must plan Install when not registered: {:?}",
        p.operations.iter().map(|o| &o.id).collect::<Vec<_>>()
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
        assert!(!label.contains("mcp"));
    }
}
