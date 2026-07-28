//! Write-ahead transactional apply with automatic rollback.

use std::path::PathBuf;

use moraine_core::init_project;
use uuid::Uuid;

use crate::agent::{adapter_for, BackupRecorder};
use crate::error::{ProvisionError, Result};
use crate::journal;
use crate::service::ServiceManager;
use crate::service_ready::{default_service_probe, ServiceProbe};
use crate::snapshot::{durable_backup, optional_file_sha256, restore_snapshot, snapshot_absent};
use crate::suite::SuitePaths;
use crate::types::{
    ApplyOutcome, CompletedOperation, FileSnapshot, ProvisionOpKind, Readiness, ServiceSnapshot,
    SetupPlan, SetupReceipt, SetupStateWitness, VerificationMode,
};
use crate::verify::{self, VerifyOptions};

/// Journaled snapshot recorder: each snapshot is fsynced into the transaction
/// journal **before** the caller mutates the original path.
pub struct JournaledBackupRecorder<'a> {
    receipt: &'a mut SetupReceipt,
}

impl<'a> JournaledBackupRecorder<'a> {
    pub fn new(receipt: &'a mut SetupReceipt) -> Self {
        Self { receipt }
    }
}

impl BackupRecorder for JournaledBackupRecorder<'_> {
    fn record_snapshot(&mut self, snapshot: FileSnapshot) -> Result<()> {
        self.receipt.snapshots.push(snapshot);
        journal::write_journal(self.receipt)?;
        Ok(())
    }
}

/// Apply a plan with write-ahead journaling and automatic rollback on failure.
pub fn apply(plan: SetupPlan, service: &dyn ServiceManager) -> Result<ApplyOutcome> {
    apply_with_options(plan, service, None, None)
}

/// Apply with optional verify overrides (tests inject service probe / capture).
pub fn apply_with_options(
    plan: SetupPlan,
    service: &dyn ServiceManager,
    verify_opts: Option<VerifyOptions>,
    service_probe: Option<std::sync::Arc<dyn ServiceProbe>>,
) -> Result<ApplyOutcome> {
    let current = compute_witness(&plan.intent, service, &plan.absolute_cli)?;
    if current != plan.state_witness {
        return Err(ProvisionError::msg(
            "setup plan is stale — system state changed; re-plan before applying",
        ));
    }

    let transaction_id = Uuid::new_v4();
    let journal_path = journal::journal_path(transaction_id);
    let mut receipt = SetupReceipt {
        transaction_id,
        intent: plan.intent.clone(),
        completed: Vec::new(),
        snapshots: Vec::new(),
        service_prestate: None,
        transaction_enabled_autostart: false,
        transaction_started_service: false,
        transaction_wrote_unit: false,
        transaction_initialized_project: false,
        readiness: Readiness::NotConfigured,
        failed_operation: None,
        error: None,
        retained_changes: Vec::new(),
        journal_path: journal_path.display().to_string(),
    };
    journal::write_journal(&receipt)?;

    let suite = SuitePaths::discover();
    let absolute_cli = PathBuf::from(&plan.absolute_cli);
    let probe = service_probe.unwrap_or_else(default_service_probe);

    for op in &plan.operations {
        let result: Result<String> = (|| {
            match op.kind {
                ProvisionOpKind::InitializeProject => {
                    let r = init_project(Some(&plan.intent.project))
                        .map_err(|e| ProvisionError::msg(e.to_string()))?;
                    receipt.transaction_initialized_project = r.created;
                    Ok(format!(
                        "project ready at {} (created={})",
                        r.project_root.display(),
                        r.created
                    ))
                }
                ProvisionOpKind::ConfigureAgent => {
                    let adapter = adapter_for(plan.intent.agent);
                    let install_plan = adapter.plan_install(&plan.intent.project, &absolute_cli)?;
                    let mut recorder = JournaledBackupRecorder::new(&mut receipt);
                    let install_receipt = adapter.apply(&install_plan, &mut recorder)?;
                    Ok(install_receipt.actions.join("; "))
                }
                ProvisionOpKind::InstallService => {
                    capture_service_prestate(&mut receipt, service, &suite)?;
                    let bin = suite
                        .absolute_service()
                        .or_else(|| {
                            absolute_cli
                                .parent()
                                .map(|p| p.join("moraine-service"))
                                .filter(|p| p.is_file())
                        })
                        .ok_or_else(|| ProvisionError::msg("service binary not found in suite"))?;

                    // Write-ahead mutation marker: install may mutate the unit and
                    // still return an error, so rollback must treat the attempt as real.
                    receipt.transaction_wrote_unit = true;
                    journal::write_journal(&receipt)?;
                    service.install(&bin)?;
                    Ok(format!("installed service from {}", bin.display()))
                }
                ProvisionOpKind::EnableAutostart => {
                    capture_service_prestate(&mut receipt, service, &suite)?;
                    let already = receipt
                        .service_prestate
                        .as_ref()
                        .map(|s| s.autostart_was_enabled)
                        .unwrap_or(false);
                    if already {
                        Ok("autostart already enabled (no-op)".into())
                    } else {
                        receipt.transaction_enabled_autostart = true;
                        journal::write_journal(&receipt)?;
                        service.enable_autostart()?;
                        Ok("autostart enabled".into())
                    }
                }
                ProvisionOpKind::StartService => {
                    capture_service_prestate(&mut receipt, service, &suite)?;
                    let was_running = receipt
                        .service_prestate
                        .as_ref()
                        .map(|s| s.was_running)
                        .unwrap_or(false);
                    if was_running && !receipt.transaction_wrote_unit {
                        // Already running and we did not rewrite the unit — leave it.
                        Ok("service already running (no-op)".into())
                    } else {
                        if !was_running {
                            receipt.transaction_started_service = true;
                        }
                        journal::write_journal(&receipt)?;
                        service.start()?;

                        // `start` succeeded. The write-ahead flag above must remain
                        // set while readiness is probed so a timeout stops the service.
                        let ready = probe
                            .wait_ready(crate::service_ready::default_service_ready_timeout_ms());
                        if ready.ready {
                            Ok(format!("service started ({})", ready.message))
                        } else {
                            Err(ProvisionError::msg(ready.message))
                        }
                    }
                }
                ProvisionOpKind::SelfTest => {
                    let mode = if plan.intent.skip_service {
                        VerificationMode::DirectCoreTest
                    } else {
                        VerificationMode::ProductCapture
                    };
                    let opts = verify_opts.clone().unwrap_or_else(|| VerifyOptions {
                        mode,
                        capture: None,
                        service_probe: Some(probe.clone()),
                    });
                    let opts = VerifyOptions {
                        mode,
                        capture: opts.capture,
                        service_probe: opts.service_probe.or(Some(probe.clone())),
                    };
                    let report = verify::verify_with_options(&plan.intent, opts)?;
                    if report.readiness == Readiness::Ready
                        || report.readiness == Readiness::DirectVerified
                    {
                        Ok(report.user_message)
                    } else {
                        Err(ProvisionError::msg(report.user_message))
                    }
                }
            }
        })();

        match result {
            Ok(msg) => {
                receipt.completed.push(CompletedOperation {
                    id: op.id.clone(),
                    kind: op.kind,
                    product_label: op.product_label.clone(),
                    success: true,
                    message: Some(msg),
                    technical_detail: None,
                });
                if let Err(error) = journal::write_journal(&receipt) {
                    let error = format!("failed to journal completed operation {}: {error}", op.id);
                    receipt.failed_operation = Some(op.id.clone());
                    receipt.error = Some(error.clone());
                    receipt.readiness = Readiness::RollbackRequired;
                    return Ok(auto_rollback(receipt, service, error));
                }
            }
            Err(error) => {
                let err = error.to_string();
                receipt.completed.push(CompletedOperation {
                    id: op.id.clone(),
                    kind: op.kind,
                    product_label: op.product_label.clone(),
                    success: false,
                    message: Some(err.clone()),
                    technical_detail: Some(err.clone()),
                });
                receipt.failed_operation = Some(op.id.clone());
                receipt.error = Some(err.clone());
                receipt.readiness = Readiness::RollbackRequired;
                if let Err(je) = journal::write_journal(&receipt) {
                    let rb = auto_rollback(receipt, service, err);
                    return match rb {
                        ApplyOutcome::RolledBack {
                            receipt,
                            original_error,
                        } => Ok(ApplyOutcome::RolledBack {
                            receipt,
                            original_error: format!(
                                "{original_error}; journal_error_on_failure={je}"
                            ),
                        }),
                        other => Ok(other),
                    };
                }
                return Ok(auto_rollback(receipt, service, err));
            }
        }
    }

    receipt.readiness = if plan.intent.skip_service {
        Readiness::DirectVerified
    } else {
        Readiness::Ready
    };
    if let Err(error) = journal::write_journal(&receipt) {
        let error = format!("failed to journal completed setup: {error}");
        receipt.failed_operation = Some("finalize".into());
        receipt.error = Some(error.clone());
        receipt.readiness = Readiness::RollbackRequired;
        return Ok(auto_rollback(receipt, service, error));
    }

    if plan.intent.skip_service {
        Ok(ApplyOutcome::DirectVerified { receipt })
    } else {
        Ok(ApplyOutcome::Ready { receipt })
    }
}

/// Unit/registration path the active ServiceManager will install or restore.
/// Prefers `inspect().unit_path` so hermetic managers (temp unit) and Linux
/// (suite unit) stay consistent with what `install` actually writes.
fn capture_service_prestate(
    receipt: &mut SetupReceipt,
    service: &dyn ServiceManager,
    suite: &SuitePaths,
) -> Result<()> {
    if receipt.service_prestate.is_some() {
        return Ok(());
    }
    let st = service.inspect()?;
    let unit_path = st
        .unit_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| suite.unit.clone());
    let registration = if unit_path.is_file() {
        durable_backup(&unit_path)?
    } else {
        snapshot_absent(&unit_path)
    };
    // Journal unit backup into snapshots as well when Existing (already durable_backup).
    if matches!(registration, FileSnapshot::Existing { .. }) {
        // Already written to disk by durable_backup; also track on receipt snapshots if not duplicate.
        if !receipt
            .snapshots
            .iter()
            .any(|s| s.path() == registration.path())
        {
            receipt.snapshots.push(registration.clone());
        }
    }
    receipt.service_prestate = Some(ServiceSnapshot {
        registration,
        was_running: st.running,
        autostart_was_enabled: st.autostart_enabled,
    });
    journal::write_journal(receipt)?;
    Ok(())
}

fn auto_rollback(
    mut receipt: SetupReceipt,
    service: &dyn ServiceManager,
    original_error: String,
) -> ApplyOutcome {
    match rollback_completed_operations(&receipt, service) {
        Ok(retained) => {
            receipt.retained_changes = retained;
            receipt.readiness = if receipt.retained_changes.is_empty() {
                Readiness::Failed
            } else {
                Readiness::Degraded
            };
            receipt.error = Some(format!("rolled back after: {original_error}"));
            if let Err(e) = journal::write_journal(&receipt) {
                return ApplyOutcome::RollbackRequired {
                    receipt,
                    original_error,
                    rollback_error: format!("ops reversed but journal failed: {e}"),
                };
            }
            ApplyOutcome::RolledBack {
                receipt,
                original_error,
            }
        }
        Err(e) => {
            receipt.readiness = Readiness::RollbackRequired;
            let rollback_error = e.to_string();
            receipt.error = Some(format!(
                "rollback failed after {original_error}: {rollback_error}"
            ));
            let _ = journal::write_journal(&receipt);
            ApplyOutcome::RollbackRequired {
                receipt,
                original_error,
                rollback_error,
            }
        }
    }
}

/// Shared rollback for automatic and manual recovery — restores exact prestate.
pub fn rollback_completed_operations(
    receipt: &SetupReceipt,
    service: &dyn ServiceManager,
) -> Result<Vec<String>> {
    let mut retained = Vec::new();
    if receipt.transaction_initialized_project {
        retained.push("Project records were retained to avoid deleting ledger data.".into());
    }
    let pre = receipt.service_prestate.as_ref();

    // Phase 1: reverse transaction-created autostart while registration still exists.
    if receipt.transaction_enabled_autostart {
        service.disable_autostart()?;
    }

    // Phase 2: stop any process that may use transaction-written registration.
    if receipt.transaction_started_service
        || (receipt.transaction_wrote_unit && pre.is_some_and(|p| p.was_running))
    {
        service.stop()?;
    }

    // Phase 3: restore/remove registration and reload the manager's cached definition.
    if receipt.transaction_wrote_unit {
        if let Some(pre) = pre {
            match &pre.registration {
                FileSnapshot::Existing { .. } => {
                    restore_snapshot(&pre.registration)?;
                    // Critical: systemd will keep the repaired unit until daemon-reload.
                    service.reload_registration()?;
                }
                FileSnapshot::Absent { .. } => {
                    service.uninstall()?;
                }
            }
        } else {
            service.uninstall()?;
        }
    }

    // Phase 4: restore prior running state only after exact registration restoration.
    if pre.map(|p| p.was_running).unwrap_or(false) {
        service.start()?;
    }

    // Phase 5: project file snapshots (Codex etc.) — skip unit path already restored.
    for snap in receipt.snapshots.iter().rev() {
        if let Some(pre) = pre {
            if snap.path() == pre.registration.path() {
                continue;
            }
        }
        restore_snapshot(snap)?;
    }

    Ok(retained)
}

/// Manual / public rollback API.
pub fn rollback(receipt: SetupReceipt, service: &dyn ServiceManager) -> Result<()> {
    let retained = rollback_completed_operations(&receipt, service)?;
    let mut updated = receipt;
    updated.readiness = if retained.is_empty() {
        Readiness::Failed
    } else {
        Readiness::Degraded
    };
    updated.retained_changes = retained;
    updated.error = Some("rolled back".into());
    journal::write_journal(&updated)?;
    Ok(())
}

pub fn apply_default(plan: SetupPlan) -> Result<ApplyOutcome> {
    let svc = crate::service::default_service_manager();
    apply(plan, svc.as_ref())
}

pub fn rollback_default(receipt: SetupReceipt) -> Result<()> {
    let svc = crate::service::default_service_manager();
    rollback(receipt, svc.as_ref())
}

pub fn apply_receipt(plan: SetupPlan, service: &dyn ServiceManager) -> Result<SetupReceipt> {
    let outcome = apply(plan, service)?;
    Ok(outcome.receipt().clone())
}

pub fn compute_witness(
    intent: &crate::types::SetupIntent,
    service: &dyn ServiceManager,
    absolute_cli: &str,
) -> Result<SetupStateWitness> {
    let initialized = moraine_core::resolve_existing_project(Some(&intent.project)).is_ok();
    let st = service.inspect()?;
    let suite = SuitePaths::discover();
    let suite_version = suite.read_manifest().map(|m| m.version).unwrap_or_default();
    let suite_cli_hash =
        optional_file_sha256(std::path::Path::new(absolute_cli)).unwrap_or_default();
    let cfg = intent.project.join(".codex/config.toml");
    let hooks = intent.project.join(".codex/hooks.json");
    let unit = st
        .unit_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| suite.unit.clone());
    Ok(SetupStateWitness {
        project: intent.project.display().to_string(),
        absolute_cli: absolute_cli.to_string(),
        suite_version,
        suite_cli_hash,
        codex_config_hash: optional_file_sha256(&cfg),
        codex_hooks_hash: optional_file_sha256(&hooks),
        service_unit_hash: optional_file_sha256(&unit),
        project_initialized: initialized,
        service_installed: st.installed,
        service_registration_valid: st.registration_valid,
        service_running: st.running,
        enable_autostart: intent.enable_autostart,
        skip_service: intent.skip_service,
    })
}
