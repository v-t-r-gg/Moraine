//! Write-ahead transactional apply with automatic rollback.

use std::path::PathBuf;

use moraine_core::init_project;
use uuid::Uuid;

use crate::agent::{adapter_for, BackupRecorder};
use crate::error::{ProvisionError, Result};
use crate::journal;
use crate::service::ServiceManager;
use crate::service_ready::{default_service_probe, ServiceProbe};
use crate::suite::SuitePaths;
use crate::types::{
    ApplyOutcome, BackupRecord, CompletedOperation, ProvisionOpKind, Readiness, SetupPlan,
    SetupReceipt, SetupStateWitness, VerificationMode,
};
use crate::verify::{self, VerifyOptions};

/// Journaled backup recorder: each backup is fsynced into the transaction journal
/// **before** the caller mutates the original file.
pub struct JournaledBackupRecorder<'a> {
    receipt: &'a mut SetupReceipt,
}

impl<'a> JournaledBackupRecorder<'a> {
    pub fn new(receipt: &'a mut SetupReceipt) -> Self {
        Self { receipt }
    }
}

impl BackupRecorder for JournaledBackupRecorder<'_> {
    fn record_backup(&mut self, backup: BackupRecord) -> Result<()> {
        self.receipt.backups.push(backup);
        // Required write boundary — fail the transaction if journal cannot be persisted.
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
    // Reject stale plan (state changed since user approved).
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
        backups: Vec::new(),
        readiness: Readiness::NotConfigured,
        failed_operation: None,
        error: None,
        journal_path: journal_path.display().to_string(),
    };
    // Required write-ahead: fail transaction if journal cannot be persisted.
    journal::write_journal(&receipt)?;

    let suite = SuitePaths::discover();
    let absolute_cli = PathBuf::from(&plan.absolute_cli);
    let probe = service_probe.unwrap_or_else(default_service_probe);

    for op in &plan.operations {
        let result = match op.kind {
            ProvisionOpKind::InitializeProject => {
                match init_project(Some(&plan.intent.project)) {
                    Ok(r) => Ok(format!(
                        "project ready at {} (created={})",
                        r.project_root.display(),
                        r.created
                    )),
                    Err(e) => Err(e.to_string()),
                }
            }
            ProvisionOpKind::ConfigureAgent => {
                let adapter = adapter_for(plan.intent.agent);
                match adapter.plan_install(&plan.intent.project, &absolute_cli) {
                    Ok(ip) => {
                        let mut recorder = JournaledBackupRecorder::new(&mut receipt);
                        match adapter.apply(&ip, &mut recorder) {
                            Ok(ir) => Ok(ir.actions.join("; ")),
                            Err(e) => Err(e.to_string()),
                        }
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
            ProvisionOpKind::InstallService => {
                let bin = suite.absolute_service().or_else(|| {
                    absolute_cli
                        .parent()
                        .map(|p| p.join("moraine-service"))
                        .filter(|p| p.is_file())
                });
                match bin {
                    Some(bin) => match service.install(&bin) {
                        Ok(()) => Ok(format!("installed service from {}", bin.display())),
                        Err(e) => Err(e.to_string()),
                    },
                    None => Err("service binary not found in suite".into()),
                }
            }
            ProvisionOpKind::EnableAutostart => match service.enable_autostart() {
                Ok(()) => Ok("autostart enabled".into()),
                Err(e) => Err(e.to_string()),
            },
            ProvisionOpKind::StartService => match service.start() {
                Ok(()) => {
                    let ready = probe.wait_ready(
                        crate::service_ready::default_service_ready_timeout_ms(),
                    );
                    if ready.ready {
                        Ok(format!("service started ({})", ready.message))
                    } else {
                        let st = service.inspect().ok();
                        if st.as_ref().map(|s| s.platform == "memory").unwrap_or(false)
                            && st.as_ref().map(|s| s.running).unwrap_or(false)
                        {
                            Ok("service started (memory manager)".into())
                        } else {
                            Err(ready.message)
                        }
                    }
                }
                Err(e) => Err(e.to_string()),
            },
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
                match verify::verify_with_options(&plan.intent, opts) {
                    Ok(report)
                        if report.readiness == Readiness::Ready
                            || report.readiness == Readiness::DirectVerified =>
                    {
                        Ok(report.user_message)
                    }
                    Ok(report) => Err(report.user_message),
                    Err(e) => Err(e.to_string()),
                }
            }
        };

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
                journal::write_journal(&receipt)?;
            }
            Err(err) => {
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
                // Required journal of failure state (backups already journaled if any).
                if let Err(je) = journal::write_journal(&receipt) {
                    // Still attempt rollback; surface journal error if rollback also fails.
                    let rb = auto_rollback(receipt, service, err);
                    return match rb {
                        ApplyOutcome::RolledBack { receipt, original_error } => {
                            Ok(ApplyOutcome::RolledBack {
                                receipt,
                                original_error: format!(
                                    "{original_error}; journal_error_on_failure={je}"
                                ),
                            })
                        }
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
    journal::write_journal(&receipt)?;

    if plan.intent.skip_service {
        Ok(ApplyOutcome::DirectVerified { receipt })
    } else {
        Ok(ApplyOutcome::Ready { receipt })
    }
}

fn auto_rollback(
    mut receipt: SetupReceipt,
    service: &dyn ServiceManager,
    original_error: String,
) -> ApplyOutcome {
    match rollback_snapshots_only(&receipt, service) {
        Ok(()) => {
            receipt.readiness = Readiness::Failed;
            receipt.error = Some(format!("rolled back after: {original_error}"));
            // Required: persist rolled-back state.
            if let Err(e) = journal::write_journal(&receipt) {
                return ApplyOutcome::RollbackRequired {
                    receipt,
                    original_error,
                    rollback_error: format!("snapshot restore ok but journal failed: {e}"),
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

/// Restore exact file snapshots only. Does **not** run semantic agent.remove.
pub fn rollback(receipt: SetupReceipt, service: &dyn ServiceManager) -> Result<()> {
    rollback_snapshots_only(&receipt, service)?;
    for op in receipt.completed.iter().rev() {
        if !op.success {
            continue;
        }
        match op.kind {
            ProvisionOpKind::StartService => {
                let _ = service.stop();
            }
            ProvisionOpKind::InstallService => {
                let _ = service.uninstall();
            }
            ProvisionOpKind::EnableAutostart
            | ProvisionOpKind::InitializeProject
            | ProvisionOpKind::ConfigureAgent
            | ProvisionOpKind::SelfTest => {}
        }
    }
    let mut updated = receipt;
    updated.readiness = Readiness::Failed;
    updated.error = Some("rolled back".into());
    journal::write_journal(&updated)?;
    Ok(())
}

fn rollback_snapshots_only(receipt: &SetupReceipt, _service: &dyn ServiceManager) -> Result<()> {
    for bak in receipt.backups.iter().rev() {
        let original = PathBuf::from(&bak.original_path);
        let backup = PathBuf::from(&bak.backup_path);
        if backup.is_file() {
            if let Some(parent) = original.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&backup, &original)?;
        }
    }
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
    Ok(SetupStateWitness {
        project: intent.project.display().to_string(),
        absolute_cli: absolute_cli.to_string(),
        project_initialized: initialized,
        service_installed: st.installed,
        service_running: st.running,
        enable_autostart: intent.enable_autostart,
        skip_service: intent.skip_service,
    })
}
