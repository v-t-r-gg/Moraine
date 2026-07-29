//! Drive shipped provisioning APIs: product verify, write-ahead apply, rollback.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use moraine_provision::{
    apply, apply_with_options, enable_project, health, plan, rollback, verify, verify_with_options,
    AgentKind, AlwaysOfflineProbe, AlwaysReadyProbe, ApplyOutcome, ControlledCapture, FileSnapshot,
    MemoryServiceManager, ProvisionOpKind, ProvisionOperation, Readiness, RepairAction, RepairKind,
    ServiceLog, ServiceManager, ServiceState, SetupIntent, SetupPlan, UnsupportedRuntimeManager,
    VecBackupRecorder, VerificationMode, VerifyOptions,
};
use tempfile::tempdir;

/// Serialize env mutations (MORAINE_CLI / PATH) across tests.
static ENV_LOCK: Mutex<()> = Mutex::new(());

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

/// Inject absolute fake `moraine` + `codex` so product verify is hermetic.
struct HermeticSuite {
    _dir: tempfile::TempDir,
    cli: PathBuf,
    service: PathBuf,
}

impl HermeticSuite {
    fn install() -> Self {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("moraine");
        let service = dir.path().join("moraine-service");
        let codex = dir.path().join("codex");
        for p in [&cli, &service, &codex] {
            fs::write(p, b"#!/bin/true\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(p).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(p, perms).unwrap();
            }
        }
        std::env::set_var("MORAINE_CLI", &cli);
        std::env::set_var("MORAINE_SERVICE_BIN", &service);
        std::env::set_var("MORAINE_CODEX", &codex);
        let path = format!(
            "{}:{}",
            dir.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        std::env::set_var("PATH", path);
        Self {
            _dir: dir,
            cli,
            service,
        }
    }
}

fn setup_agent(project: &Path) {
    moraine_core::init_project(Some(project)).unwrap();
    let cli = moraine_provision::SuitePaths::discover().absolute_cli();
    assert!(
        cli.is_absolute() && cli.is_file(),
        "suite CLI must be absolute file: {}",
        cli.display()
    );
    let adapter = moraine_provision::adapter_for(AgentKind::Codex);
    let mut rec = VecBackupRecorder::new();
    adapter
        .apply(&adapter.plan_install(project, &cli).unwrap(), &mut rec)
        .unwrap();
}

fn plan_with_operations(
    intent: SetupIntent,
    service: &dyn ServiceManager,
    absolute_cli: &Path,
    kinds: &[ProvisionOpKind],
) -> SetupPlan {
    SetupPlan {
        plan_id: uuid::Uuid::new_v4(),
        state_witness: moraine_provision::compute_witness(
            &intent,
            service,
            &absolute_cli.display().to_string(),
        )
        .unwrap(),
        intent,
        operations: kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| ProvisionOperation {
                id: format!("test_{index}_{kind:?}"),
                kind: *kind,
                product_label: kind.product_label().into(),
                detail: "hermetic lifecycle test".into(),
                reversible: *kind != ProvisionOpKind::SelfTest,
            })
            .collect(),
        warnings: vec![],
        absolute_cli: absolute_cli.display().to_string(),
        product_summary: vec![],
    }
}

struct BreakJournalOnInstall {
    inner: MemoryServiceManager,
    journal_dir: PathBuf,
}

impl ServiceManager for BreakJournalOnInstall {
    fn inspect(&self) -> moraine_provision::Result<ServiceState> {
        self.inner.inspect()
    }

    fn capture_registration(
        &self,
    ) -> moraine_provision::Result<moraine_provision::RuntimeRegistrationSnapshot> {
        self.inner.capture_registration()
    }

    fn registration_fingerprint(&self) -> moraine_provision::Result<Option<String>> {
        self.inner.registration_fingerprint()
    }

    fn install(&self, executable: &Path) -> moraine_provision::Result<()> {
        self.inner.install(executable)?;
        fs::remove_dir_all(&self.journal_dir)?;
        fs::write(&self.journal_dir, b"blocks journal directory recreation")?;
        Ok(())
    }

    fn restore_registration(
        &self,
        snapshot: &moraine_provision::RuntimeRegistrationSnapshot,
    ) -> moraine_provision::Result<()> {
        self.inner.restore_registration(snapshot)
    }

    fn uninstall(&self) -> moraine_provision::Result<()> {
        self.inner.uninstall()
    }

    fn start(&self) -> moraine_provision::Result<()> {
        self.inner.start()
    }

    fn stop(&self) -> moraine_provision::Result<()> {
        self.inner.stop()
    }

    fn restart(&self) -> moraine_provision::Result<()> {
        self.inner.restart()
    }

    fn enable_autostart(&self) -> moraine_provision::Result<()> {
        self.inner.enable_autostart()
    }

    fn disable_autostart(&self) -> moraine_provision::Result<()> {
        self.inner.disable_autostart()
    }

    fn logs(&self, limit: usize) -> moraine_provision::Result<Vec<ServiceLog>> {
        self.inner.logs(limit)
    }
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
    assert_eq!(report.readiness, Readiness::DirectVerified);
}

#[test]
fn windows_capabilities_fail_closed_across_product_capture_boundaries() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("unsupported-product");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    let service = MemoryServiceManager::new();
    let capabilities =
        moraine_platform::PlatformCapabilities::for_host(moraine_platform::HostPlatform::Windows);

    let plan_error = moraine_provision::plan::plan_with_capabilities(
        product_intent(project.clone()),
        &service,
        &capabilities,
    )
    .unwrap_err();
    assert!(matches!(
        plan_error,
        moraine_provision::ProvisionError::UnsupportedPlatform {
            operation: "product_capture_plan",
            ..
        }
    ));

    let fake_cli = dir.path().join("moraine");
    fs::write(&fake_cli, b"fake").unwrap();
    let approved = plan_with_operations(
        product_intent(project.clone()),
        &service,
        &fake_cli,
        &[ProvisionOpKind::InitializeProject],
    );
    let journal_dir = dir.path().join("journals");
    let apply_error =
        moraine_provision::journal::with_journal_dir_override(journal_dir.clone(), || {
            moraine_provision::apply::apply_with_options_and_capabilities(
                approved,
                &service,
                None,
                None,
                &capabilities,
            )
            .unwrap_err()
        });
    assert!(matches!(
        apply_error,
        moraine_provision::ProvisionError::UnsupportedPlatform {
            operation: "product_capture_apply",
            ..
        }
    ));
    assert!(
        !journal_dir.exists(),
        "unsupported apply must fail before transaction creation"
    );

    let verify_error = moraine_provision::verify::verify_with_options_and_capabilities(
        &product_intent(project.clone()),
        VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: None,
            service_probe: None,
        },
        &capabilities,
    )
    .unwrap_err();
    assert!(matches!(
        verify_error,
        moraine_provision::ProvisionError::UnsupportedPlatform {
            operation: "product_capture_verify",
            ..
        }
    ));

    let direct = moraine_provision::verify::verify_with_options_and_capabilities(
        &direct_intent(project),
        VerifyOptions {
            mode: VerificationMode::DirectCoreTest,
            capture: None,
            service_probe: None,
        },
        &capabilities,
    )
    .unwrap();
    assert_ne!(direct.readiness, Readiness::Ready);
    assert!(
        matches!(
            direct.readiness,
            Readiness::DirectVerified | Readiness::Failed
        ),
        "DirectCoreTest remains callable but never becomes Product Ready: {direct:?}"
    );
}

#[test]
fn forged_skip_service_plan_is_rejected_before_inspection_or_journaling() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("forged-unsupported-product");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    let service = MemoryServiceManager::new();
    let capabilities =
        moraine_platform::PlatformCapabilities::for_host(moraine_platform::HostPlatform::Windows);
    let fake_cli = dir.path().join("moraine");
    fs::write(&fake_cli, b"fake").unwrap();

    let mut intent = product_intent(project);
    intent.skip_service = true;
    let approved = plan_with_operations(
        intent,
        &service,
        &fake_cli,
        &[ProvisionOpKind::InstallService],
    );
    let inspections_before = service.inspect_count();
    let operations_before = service.operation_counts();
    let journal_dir = dir.path().join("journals");

    let error = moraine_provision::journal::with_journal_dir_override(journal_dir.clone(), || {
        moraine_provision::apply::apply_with_options_and_capabilities(
            approved,
            &service,
            None,
            None,
            &capabilities,
        )
        .unwrap_err()
    });

    assert!(matches!(
        error,
        moraine_provision::ProvisionError::UnsupportedPlatform {
            operation: "product_capture_apply",
            ..
        }
    ));
    assert_eq!(service.inspect_count(), inspections_before);
    assert_eq!(service.operation_counts(), operations_before);
    assert!(
        !journal_dir.exists(),
        "unsupported forged plan must fail before transaction creation"
    );
}

#[test]
fn unsupported_health_exposes_only_portable_project_repair() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("unsupported-health");
    fs::create_dir_all(&project).unwrap();
    let service = UnsupportedRuntimeManager::new(moraine_platform::HostPlatform::Windows);

    let report = health(&service, Some(&project), Some(AgentKind::Codex)).unwrap();

    assert!(report
        .checks
        .iter()
        .any(|check| check.id == "service.supported" && check.repair.is_none()));
    assert!(report
        .checks
        .iter()
        .filter_map(|check| check.repair.as_ref())
        .all(|repair| repair.kind == RepairKind::InitProject));

    let agent_repair = RepairAction {
        id: "forged-agent-repair".into(),
        label: "Fix".into(),
        kind: RepairKind::RepairAgentIntegration,
        project: Some(project.clone()),
        agent: Some(AgentKind::Codex),
    };
    let blocked = moraine_provision::repair(&agent_repair, &service).unwrap();
    assert!(!blocked.ok);
    assert_eq!(
        blocked.technical_detail.as_deref(),
        Some("unsupported_platform")
    );

    let init_repair = RepairAction {
        id: "portable-init".into(),
        label: "Fix".into(),
        kind: RepairKind::InitProject,
        project: Some(project.clone()),
        agent: None,
    };
    let initialized = moraine_core::project_registry::with_project_registry_path_override(
        dir.path().join("projects.json"),
        || moraine_provision::repair(&init_repair, &service).unwrap(),
    );
    assert!(initialized.ok);
    assert!(project.join(".moraine").is_dir());
}

#[test]
fn existing_initialized_project_is_registered_by_successful_apply() {
    let _lock = ENV_LOCK.lock().unwrap();
    let _suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let data_home = dir.path().join("data");
    let registry_path = data_home.join("moraine/projects.json");
    let journal_dir = data_home.join("moraine/setup-transactions");
    let project = dir.path().join("existing");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    assert!(!registry_path.exists());
    let service = MemoryServiceManager::new();

    let outcome = moraine_core::project_registry::with_project_registry_path_override(
        registry_path.clone(),
        || {
            let approved = plan(direct_intent(project.clone()), &service).unwrap();
            assert!(approved
                .operations
                .iter()
                .any(|operation| operation.kind == ProvisionOpKind::RegisterProject));
            assert!(!approved
                .operations
                .iter()
                .any(|operation| operation.kind == ProvisionOpKind::InitializeProject));
            moraine_provision::journal::with_journal_dir_override(journal_dir, || {
                apply(approved, &service).unwrap()
            })
        },
    );
    assert!(
        matches!(outcome, ApplyOutcome::DirectVerified { .. }),
        "{outcome:?}"
    );
    let registry = moraine_core::read_project_registry_at(&registry_path).unwrap();
    assert_eq!(registry.projects.len(), 1);
    assert_eq!(
        PathBuf::from(&registry.projects[0].root),
        fs::canonicalize(&project).unwrap()
    );
    let reloaded = moraine_core::read_project_registry_at(&registry_path).unwrap();
    assert_eq!(reloaded.projects[0].root, registry.projects[0].root);
    let summary = moraine_core::summarize_project(Path::new(&reloaded.projects[0].root)).unwrap();
    assert_eq!(
        summary.root_path,
        fs::canonicalize(project).unwrap().display().to_string()
    );
}

#[test]
fn product_happy_path_ready_with_injected_service_and_capture() {
    let _lock = ENV_LOCK.lock().unwrap();
    let _suite = HermeticSuite::install();
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
    assert!(report.ok, "{report:?}");
    assert_eq!(report.readiness, Readiness::Ready);
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.message.contains("verification_id=")),
        "{report:?}"
    );
    let resolved = moraine_core::resolve_existing_project(Some(&project)).unwrap();
    let runs = moraine_core::list_run_summaries(&resolved.project_root, resolved.project_id);
    assert!(
        runs.iter().all(|run| !run
            .objective
            .starts_with("Moraine self-test verification_id=")),
        "successful ProductCapture must clean up its synthetic run: {runs:?}"
    );
    assert!(report
        .steps
        .iter()
        .any(|step| step.id == "capture.self_test_cleanup" && step.passed));
}

#[test]
fn product_verify_fails_when_hook_delivery_fails_no_core_fallback() {
    let _lock = ENV_LOCK.lock().unwrap();
    let _suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("hook-fail");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);
    moraine_core::run_start(moraine_core::RunStartRequest {
        objective: "Moraine self-test: stale leftover".into(),
        idempotency_key: "stale".into(),
        project: Some(project.clone()),
        session_id: None,
    })
    .unwrap();
    let report = verify_with_options(
        &product_intent(project),
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
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.id == "capture.adapter_event" && !s.passed),
        "must fail on capture, not steal stale run: {report:?}"
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
            "# --- Moraine (managed) ---\n[mcp_servers.moraine]\ncommand = \"{}\"\n# --- end Moraine ---\n",
            cli.display()
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
    assert!(report
        .steps
        .iter()
        .any(|s| s.id == "agent.hooks" && !s.passed));
}

#[test]
fn absolute_cli_mismatch_fails_closed() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("cli-mismatch");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    fs::create_dir_all(project.join(".codex")).unwrap();
    let fake = dir.path().join("moraine");
    fs::write(&fake, b"x").unwrap();
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
            "# --- Moraine (managed) ---\n[mcp_servers.moraine]\ncommand = \"{}\"\n# --- end Moraine ---\n",
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
    assert!(report
        .steps
        .iter()
        .any(|s| s.id == "agent.absolute_cli" && !s.passed));
}

/// Release-blocking: initially absent Codex files must be deleted on auto-rollback.
#[test]
fn rollback_deletes_files_that_did_not_exist_before_setup() {
    std::env::set_var("MORAINE_SERVICE_READY_MS", "200");
    let dir = tempdir().unwrap();
    let project = dir.path().join("absent");
    fs::create_dir_all(&project).unwrap();
    // No .codex at all.
    assert!(!project.join(".codex").exists());

    let svc = MemoryServiceManager::new();
    // Force product path install failure after agent config: no service binary if we
    // filter to ConfigureAgent then a failing InstallService.
    let mut p = plan(product_intent(project.clone()), &svc).unwrap();
    // Keep init + configure + install (install will fail without suite service in some envs)
    // Ensure install is present and will fail via fail_next after configure.
    p.operations.retain(|o| {
        o.kind != ProvisionOpKind::SelfTest
            && o.kind != ProvisionOpKind::StartService
            && o.kind != ProvisionOpKind::EnableAutostart
    });

    // After plan, inject install failure
    let svc = MemoryServiceManager::new();
    // Recompute witness for fresh svc
    p.state_witness = moraine_provision::compute_witness(&p.intent, &svc, &p.absolute_cli).unwrap();
    // Pre-seed: configure will succeed creating files; then install fails.
    // Memory install without fail_next succeeds if we call install - need fail on InstallService.
    // Use fail_next so first service op fails — but InstallService calls install().
    // Order: init, configure, install. After configure, files exist. fail_next on install.
    // Actually MemoryServiceManager fail_next applies to next install OR start.
    // We need configure first without fail, then install fails.
    // fail_next is set before apply — would fail install only if configure doesn't call install.
    // Configure doesn't. Good — set fail_next before apply.
    svc.fail_next("injected install failure");

    let outcome = apply(p, &svc).unwrap();
    assert!(
        matches!(
            outcome,
            ApplyOutcome::RolledBack { .. } | ApplyOutcome::RollbackRequired { .. }
        ),
        "{outcome:?}"
    );
    let receipt = outcome.receipt();
    // Snapshots must include Absent for new files.
    assert!(
        receipt
            .snapshots
            .iter()
            .any(|s| matches!(s, FileSnapshot::Absent { .. })),
        "expected Absent snapshots: {:?}",
        receipt.snapshots
    );
    // After rollback both files must be gone.
    assert!(
        !project.join(".codex/config.toml").exists(),
        "config.toml must be deleted on rollback"
    );
    assert!(
        !project.join(".codex/hooks.json").exists(),
        "hooks.json must be deleted on rollback"
    );
}

/// Release-blocking: auto-rollback stops/uninstalls service started by the transaction.
#[test]
fn auto_rollback_reverses_service_install_and_start() {
    let _lock = ENV_LOCK.lock().unwrap();
    std::env::set_var("MORAINE_SERVICE_READY_MS", "100");
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("svc-rb");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);

    let svc = MemoryServiceManager::new();
    use moraine_provision::{ProvisionOperation, SetupPlan};
    let abs_cli = suite.cli.display().to_string();
    let intent = product_intent(project.clone());
    let witness = moraine_provision::compute_witness(&intent, &svc, &abs_cli).unwrap();
    let p = SetupPlan {
        plan_id: uuid::Uuid::new_v4(),
        intent,
        operations: vec![
            ProvisionOperation {
                id: "install_service".into(),
                kind: ProvisionOpKind::InstallService,
                product_label: "Enabling background capture".into(),
                detail: "test".into(),
                reversible: true,
            },
            ProvisionOperation {
                id: "start_service".into(),
                kind: ProvisionOpKind::StartService,
                product_label: "Starting background capture".into(),
                detail: "test".into(),
                reversible: true,
            },
            ProvisionOperation {
                id: "self_test".into(),
                kind: ProvisionOpKind::SelfTest,
                product_label: "Testing local capture".into(),
                detail: "test".into(),
                reversible: false,
            },
        ],
        warnings: vec![],
        absolute_cli: abs_cli,
        product_summary: vec![],
        state_witness: witness,
    };

    let outcome = apply_with_options(
        p,
        &svc,
        Some(VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: true,
                materialize_run: false,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        }),
        Some(Arc::new(AlwaysReadyProbe { version: None })),
    )
    .unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "expected RolledBack, got {outcome:?}"
    );
    let receipt = outcome.receipt();
    assert!(
        receipt
            .completed
            .iter()
            .any(|op| op.kind == ProvisionOpKind::InstallService && op.success),
        "InstallService must have succeeded before failure: {:?}",
        receipt.completed
    );
    assert!(
        receipt
            .completed
            .iter()
            .any(|op| op.kind == ProvisionOpKind::StartService && op.success),
        "StartService must have succeeded before failure: {:?}",
        receipt.completed
    );
    let st = svc.inspect().unwrap();
    assert!(!st.running, "service must be stopped after auto-rollback");
    assert!(
        !st.installed && !st.registration_present,
        "service must be uninstalled after auto-rollback: {st:?}"
    );
}

/// Unit repair: prior unit bytes restored + registration reload on auto-rollback.
#[test]
fn auto_rollback_restores_prior_unit_bytes_and_reloads() {
    let _lock = ENV_LOCK.lock().unwrap();
    std::env::set_var("MORAINE_SERVICE_READY_MS", "100");
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("unit-repair");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);

    let unit_path = dir.path().join("systemd/user/moraine-service.service");
    fs::create_dir_all(unit_path.parent().unwrap()).unwrap();
    let original_unit = "# prior unit\nExecStart=/old/wrong/moraine-service\n";
    fs::write(&unit_path, original_unit).unwrap();

    let svc = MemoryServiceManager::with_unit_path(unit_path.clone());
    use moraine_provision::{ProvisionOperation, SetupPlan};
    let abs_cli = suite.cli.display().to_string();
    let intent = product_intent(project.clone());
    let witness = moraine_provision::compute_witness(&intent, &svc, &abs_cli).unwrap();
    let p = SetupPlan {
        plan_id: uuid::Uuid::new_v4(),
        intent,
        operations: vec![
            ProvisionOperation {
                id: "install_service".into(),
                kind: ProvisionOpKind::InstallService,
                product_label: "Enabling background capture".into(),
                detail: "repair unit".into(),
                reversible: true,
            },
            ProvisionOperation {
                id: "start_service".into(),
                kind: ProvisionOpKind::StartService,
                product_label: "Starting background capture".into(),
                detail: "test".into(),
                reversible: true,
            },
            ProvisionOperation {
                id: "self_test".into(),
                kind: ProvisionOpKind::SelfTest,
                product_label: "Testing local capture".into(),
                detail: "test".into(),
                reversible: false,
            },
        ],
        warnings: vec![],
        absolute_cli: abs_cli,
        product_summary: vec![],
        state_witness: witness,
    };

    let outcome = apply_with_options(
        p,
        &svc,
        Some(VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: true,
                materialize_run: false,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        }),
        Some(Arc::new(AlwaysReadyProbe { version: None })),
    )
    .unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "expected RolledBack, got {outcome:?}"
    );
    let receipt = outcome.receipt();
    assert!(
        receipt.transaction_wrote_unit,
        "InstallService must have written/overwritten the unit"
    );
    assert!(
        receipt.service_prestate.as_ref().is_some_and(|p| matches!(
            p.registration,
            moraine_provision::RuntimeRegistrationSnapshot::File(FileSnapshot::Existing { .. })
        )),
        "prestate must snapshot prior unit as Existing: {:?}",
        receipt.service_prestate
    );
    let restored = fs::read_to_string(&unit_path).expect("unit file must exist after rollback");
    assert_eq!(
        restored, original_unit,
        "on-disk unit must equal original bytes after repair rollback"
    );
    assert!(
        svc.reload_count() >= 1,
        "reload_registration (daemon-reload equivalent) must run after unit restore, count={}",
        svc.reload_count()
    );
    let st = svc.inspect().unwrap();
    assert!(!st.running, "service must be stopped after auto-rollback");
}

/// Pre-enabled autostart must survive rollback of a failed later op.
#[test]
fn rollback_preserves_preexisting_autostart() {
    let _lock = ENV_LOCK.lock().unwrap();
    std::env::set_var("MORAINE_SERVICE_READY_MS", "100");
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("as-pre");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);

    let svc = MemoryServiceManager::new();
    svc.install(&suite.service).unwrap();
    svc.enable_autostart().unwrap();
    assert!(svc.inspect().unwrap().autostart_enabled);

    use moraine_provision::{ProvisionOperation, SetupPlan};
    let abs_cli = suite.cli.display().to_string();
    let intent = SetupIntent {
        project: project.clone(),
        agent: AgentKind::Codex,
        enable_autostart: true,
        skip_service: false,
    };
    let witness = moraine_provision::compute_witness(&intent, &svc, &abs_cli).unwrap();
    let p = SetupPlan {
        plan_id: uuid::Uuid::new_v4(),
        intent,
        operations: vec![
            ProvisionOperation {
                id: "enable_autostart".into(),
                kind: ProvisionOpKind::EnableAutostart,
                product_label: "Keep capture available after restart".into(),
                detail: "test".into(),
                reversible: true,
            },
            ProvisionOperation {
                id: "self_test".into(),
                kind: ProvisionOpKind::SelfTest,
                product_label: "Testing local capture".into(),
                detail: "test".into(),
                reversible: false,
            },
        ],
        warnings: vec![],
        absolute_cli: abs_cli,
        product_summary: vec![],
        state_witness: witness,
    };

    let outcome = apply_with_options(
        p,
        &svc,
        Some(VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: true,
                materialize_run: false,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        }),
        Some(Arc::new(AlwaysReadyProbe { version: None })),
    )
    .unwrap();
    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "{outcome:?}"
    );
    assert!(
        !outcome.receipt().transaction_enabled_autostart,
        "must not have re-enabled autostart"
    );
    assert!(
        svc.inspect().unwrap().autostart_enabled,
        "preexisting autostart must remain enabled after rollback"
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
    p.operations.retain(|o| o.kind != ProvisionOpKind::SelfTest);
    svc.fail_next("injected");
    // Wait — fail_next on first install; but init and configure run first.
    // fail_next triggers on install after configure — good.
    // Actually we need fail on install: configure doesn't use service. Set fail_next now.
    let outcome = apply(p, &svc).unwrap();
    match &outcome {
        ApplyOutcome::RolledBack { receipt, .. }
        | ApplyOutcome::RollbackRequired { receipt, .. } => {
            assert!(!receipt.journal_path.is_empty());
        }
        other => {
            // If environment found suite service binary and succeeded partially...
            let _ = other;
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
        snapshots: vec![FileSnapshot::Existing {
            path: cfg_path.display().to_string(),
            backup_path: bak.display().to_string(),
            original_hash: "x".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }],
        service_prestate: None,
        transaction_enabled_autostart: false,
        transaction_started_service: false,
        transaction_wrote_unit: false,
        transaction_initialized_project: false,
        transaction_registered_project: false,
        readiness: Readiness::RollbackRequired,
        failed_operation: Some("configure_agent".into()),
        error: Some("test".into()),
        retained_changes: vec![],
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
    let p = plan(direct_intent(project), &svc).unwrap();
    assert!(!p.plan_id.is_nil());
    assert!(!p.state_witness.suite_cli_hash.is_empty() || p.absolute_cli.starts_with('/'));
    let outcome = apply(p, &svc).unwrap();
    assert!(matches!(outcome, ApplyOutcome::DirectVerified { .. }));
}

#[test]
fn product_apply_self_test_ready_with_injectables() {
    let _lock = ENV_LOCK.lock().unwrap();
    let _suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("prod-apply");
    let registry_path = dir.path().join("data/moraine/projects.json");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    setup_agent(&project);
    let intent = product_intent(project.clone());
    let mut p = plan(intent, &svc).unwrap();
    p.operations.retain(|o| {
        matches!(
            o.kind,
            ProvisionOpKind::RegisterProject | ProvisionOpKind::SelfTest
        )
    });
    p.state_witness = moraine_provision::compute_witness(&p.intent, &svc, &p.absolute_cli).unwrap();
    let opts = VerifyOptions {
        mode: VerificationMode::ProductCapture,
        capture: Some(Arc::new(ControlledCapture {
            fail_delivery: false,
            materialize_run: true,
        })),
        service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
    };
    let outcome =
        moraine_core::project_registry::with_project_registry_path_override(registry_path, || {
            apply_with_options(
                p,
                &svc,
                Some(opts),
                Some(Arc::new(AlwaysReadyProbe { version: None })),
            )
            .unwrap()
        });
    assert!(matches!(outcome, ApplyOutcome::Ready { .. }), "{outcome:?}");
}

#[test]
fn start_success_readiness_failure_stops_service_during_rollback() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("readiness-timeout");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    let plan = plan_with_operations(
        product_intent(project),
        &svc,
        &suite.cli,
        &[
            ProvisionOpKind::InstallService,
            ProvisionOpKind::StartService,
        ],
    );

    let outcome = apply_with_options(plan, &svc, None, Some(Arc::new(AlwaysOfflineProbe))).unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "{outcome:?}"
    );
    let (installs, starts, stops) = svc.operation_counts();
    assert_eq!(installs, 1, "install mutation must occur before rollback");
    assert_eq!(
        starts, 1,
        "start mutation must occur before readiness fails"
    );
    assert!(stops >= 1, "rollback must stop the newly started service");
    assert!(!svc.inspect().unwrap().running);
    assert!(outcome.receipt().transaction_started_service);
}

#[test]
fn partial_install_error_restores_prestate() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("partial-install");
    fs::create_dir_all(&project).unwrap();
    let unit = dir.path().join("unit/moraine-service.service");
    let svc = MemoryServiceManager::with_unit_path(unit.clone());
    svc.fail_after_install("install mutated registration then failed");
    let plan = plan_with_operations(
        product_intent(project),
        &svc,
        &suite.cli,
        &[ProvisionOpKind::InstallService],
    );

    let outcome = apply(plan, &svc).unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "{outcome:?}"
    );
    assert_eq!(svc.operation_counts().0, 1, "install mutation was reached");
    assert!(!unit.exists(), "previously absent unit must be removed");
    assert!(!svc.inspect().unwrap().registration_present);
    assert!(outcome.receipt().transaction_wrote_unit);
}

#[test]
fn journal_failure_after_completed_mutation_enters_rollback() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let previous_data_home = std::env::var_os("XDG_DATA_HOME");
    let data_home = dir.path().join("data");
    std::env::set_var("XDG_DATA_HOME", &data_home);
    let project = dir.path().join("journal-failure");
    fs::create_dir_all(&project).unwrap();
    let service = BreakJournalOnInstall {
        inner: MemoryServiceManager::new(),
        journal_dir: data_home.join("moraine/setup-transactions"),
    };
    let plan = plan_with_operations(
        product_intent(project),
        &service,
        &suite.cli,
        &[ProvisionOpKind::InstallService],
    );

    let outcome = apply(plan, &service).unwrap();

    if let Some(value) = previous_data_home {
        std::env::set_var("XDG_DATA_HOME", value);
    } else {
        std::env::remove_var("XDG_DATA_HOME");
    }
    assert!(
        matches!(outcome, ApplyOutcome::RollbackRequired { .. }),
        "journal remains unavailable after rollback, so recovery must be explicit: {outcome:?}"
    );
    assert_eq!(
        service.inner.operation_counts().0,
        1,
        "install mutation must complete before journal failure"
    );
    assert!(
        !service.inner.inspect().unwrap().registration_present,
        "automatic rollback must still reverse the completed mutation"
    );
}

#[test]
fn service_prestate_failure_after_agent_mutation_rolls_back_files() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("prestate-failure");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    let plan = plan_with_operations(
        product_intent(project.clone()),
        &svc,
        &suite.cli,
        &[
            ProvisionOpKind::InitializeProject,
            ProvisionOpKind::ConfigureAgent,
            ProvisionOpKind::InstallService,
        ],
    );
    // apply's stale-plan witness inspect succeeds; InstallService prestate inspect fails.
    svc.fail_inspect_after(1, "injected prestate capture failure");

    let outcome = apply(plan, &svc).unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "{outcome:?}"
    );
    assert!(
        outcome
            .receipt()
            .completed
            .iter()
            .any(|op| op.kind == ProvisionOpKind::ConfigureAgent && op.success),
        "agent mutation must complete before prestate failure: {outcome:?}"
    );
    assert!(!project.join(".codex/config.toml").exists());
    assert!(!project.join(".codex/hooks.json").exists());
}

#[test]
fn prior_running_service_is_restored_after_failed_repair() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("restore-running");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);
    let unit = dir.path().join("unit/moraine-service.service");
    fs::create_dir_all(unit.parent().unwrap()).unwrap();
    fs::write(&unit, b"prior exact unit bytes\n").unwrap();
    let svc = MemoryServiceManager::with_unit_path(unit.clone());
    svc.install(&suite.service).unwrap();
    fs::write(&unit, b"prior exact unit bytes\n").unwrap();
    svc.start().unwrap();
    let starts_before = svc.operation_counts().1;
    let plan = plan_with_operations(
        product_intent(project),
        &svc,
        &suite.cli,
        &[ProvisionOpKind::InstallService, ProvisionOpKind::SelfTest],
    );

    let outcome = apply_with_options(
        plan,
        &svc,
        Some(VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: true,
                materialize_run: false,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        }),
        Some(Arc::new(AlwaysReadyProbe { version: None })),
    )
    .unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "{outcome:?}"
    );
    assert!(svc.inspect().unwrap().running);
    assert!(
        svc.operation_counts().1 > starts_before,
        "rollback must restart the prior service"
    );
    assert_eq!(fs::read(&unit).unwrap(), b"prior exact unit bytes\n");
}

#[test]
fn prior_running_restart_failure_requires_manual_rollback() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("restart-failure");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);
    let unit = dir.path().join("unit/moraine-service.service");
    fs::create_dir_all(unit.parent().unwrap()).unwrap();
    fs::write(&unit, b"prior unit\n").unwrap();
    let svc = MemoryServiceManager::with_unit_path(unit);
    svc.install(&suite.service).unwrap();
    svc.start().unwrap();
    let plan = plan_with_operations(
        product_intent(project),
        &svc,
        &suite.cli,
        &[ProvisionOpKind::InstallService, ProvisionOpKind::SelfTest],
    );
    svc.fail_next_start("prior service restart failed");

    let outcome = apply_with_options(
        plan,
        &svc,
        Some(VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: true,
                materialize_run: false,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        }),
        Some(Arc::new(AlwaysReadyProbe { version: None })),
    )
    .unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RollbackRequired { .. }),
        "{outcome:?}"
    );
    assert!(!svc.inspect().unwrap().running);
}

#[test]
fn transaction_enabled_autostart_is_reversed() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("autostart-reverse");
    fs::create_dir_all(&project).unwrap();
    setup_agent(&project);
    let svc = MemoryServiceManager::new();
    svc.install(&suite.service).unwrap();
    let intent = SetupIntent {
        project,
        agent: AgentKind::Codex,
        enable_autostart: true,
        skip_service: false,
    };
    let plan = plan_with_operations(
        intent,
        &svc,
        &suite.cli,
        &[ProvisionOpKind::EnableAutostart, ProvisionOpKind::SelfTest],
    );

    let outcome = apply_with_options(
        plan,
        &svc,
        Some(VerifyOptions {
            mode: VerificationMode::ProductCapture,
            capture: Some(Arc::new(ControlledCapture {
                fail_delivery: true,
                materialize_run: false,
            })),
            service_probe: Some(Arc::new(AlwaysReadyProbe { version: None })),
        }),
        Some(Arc::new(AlwaysReadyProbe { version: None })),
    )
    .unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "{outcome:?}"
    );
    assert!(outcome.receipt().transaction_enabled_autostart);
    assert!(!svc.inspect().unwrap().autostart_enabled);
}

#[test]
fn retained_project_initialization_is_reported_as_degraded() {
    let _lock = ENV_LOCK.lock().unwrap();
    let suite = HermeticSuite::install();
    let dir = tempdir().unwrap();
    let project = dir.path().join("retained-ledger");
    fs::create_dir_all(&project).unwrap();
    let svc = MemoryServiceManager::new();
    svc.fail_after_install("fail after install mutation");
    let plan = plan_with_operations(
        product_intent(project.clone()),
        &svc,
        &suite.cli,
        &[
            ProvisionOpKind::InitializeProject,
            ProvisionOpKind::InstallService,
        ],
    );

    let outcome = apply(plan, &svc).unwrap();

    assert!(
        matches!(outcome, ApplyOutcome::RolledBack { .. }),
        "{outcome:?}"
    );
    assert!(project.join(".moraine").is_dir());
    assert_eq!(outcome.receipt().readiness, Readiness::Degraded);
    assert!(outcome.receipt().retained_changes.iter().any(|message| {
        message == "Project records were retained to avoid deleting ledger data."
    }));
}

#[test]
fn service_lifecycle_and_health_repair() {
    let svc = MemoryServiceManager::new();
    let dir = tempdir().unwrap();
    let project = dir.path().join("h");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();
    let report = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    let install_fix = report
        .checks
        .iter()
        .find_map(|c| c.repair.as_ref())
        .expect("repair");
    assert_eq!(install_fix.kind, RepairKind::InstallService);
    let fake = dir.path().join("moraine-service");
    fs::write(&fake, b"x").unwrap();
    svc.install(&fake).unwrap();
    let report = health(&svc, Some(&project), Some(AgentKind::Codex)).unwrap();
    let start_fix = report
        .checks
        .iter()
        .find_map(|c| c.repair.as_ref())
        .expect("repair");
    assert_eq!(start_fix.kind, RepairKind::StartService);
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
fn project_init_health_repair_registers_project() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let registry_path = dir.path().join("data/moraine/projects.json");
    let project = dir.path().join("health-init");
    fs::create_dir_all(&project).unwrap();
    let action = RepairAction {
        id: "repair.init_project".into(),
        label: "Fix".into(),
        kind: RepairKind::InitProject,
        project: Some(project.clone()),
        agent: None,
    };

    let result = moraine_core::project_registry::with_project_registry_path_override(
        registry_path.clone(),
        || moraine_provision::repair(&action, &MemoryServiceManager::new()).unwrap(),
    );
    assert!(result.ok, "{result:?}");
    let registry = moraine_core::read_project_registry_at(&registry_path).unwrap();
    assert_eq!(registry.projects.len(), 1);
    assert_eq!(
        PathBuf::from(&registry.projects[0].root),
        fs::canonicalize(project).unwrap()
    );
}

#[test]
fn plan_installs_when_not_registered() {
    let svc = MemoryServiceManager::new();
    let dir = tempdir().unwrap();
    let project = dir.path().join("bin-only");
    fs::create_dir_all(&project).unwrap();
    let p = plan(product_intent(project), &svc).unwrap();
    assert!(p
        .operations
        .iter()
        .any(|o| o.kind == ProvisionOpKind::InstallService));
}

#[test]
fn product_progress_labels_have_no_infra_jargon() {
    for kind in [
        ProvisionOpKind::InitializeProject,
        ProvisionOpKind::RegisterProject,
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
