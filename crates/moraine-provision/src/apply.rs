//! Write-ahead transactional apply with automatic rollback.

use std::path::PathBuf;

use moraine_core::init_project;
use uuid::Uuid;

use crate::agent::{adapter_for, BackupRecorder};
use crate::error::{ProvisionError, Result};
use crate::journal;
use crate::service::ServiceManager;
use crate::service_ready::{default_service_probe, ServiceProbe};
use crate::snapshot::{optional_file_sha256, restore_snapshot};
use crate::suite::SuitePaths;
use crate::types::{
    ApplyOutcome, CompletedOperation, FileSnapshot, ProvisionOpKind, Readiness, SetupPlan,
    SetupReceipt, SetupStateWitness, VerificationMode,
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
                    let ready = probe
                        .wait_ready(crate::service_ready::default_service_ready_timeout_ms());
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
            // Still RolledBack even with retained non-reversible notes (autostart).
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

/// Shared rollback for automatic and manual recovery.
///
/// Reverses successful completed operations (newest first), then restores file
/// snapshots (existing → copy; absent → delete).
pub fn rollback_completed_operations(
    receipt: &SetupReceipt,
    service: &dyn ServiceManager,
) -> Result<Vec<String>> {
    let mut retained = Vec::new();
    for op in receipt.completed.iter().rev() {
        if !op.success {
            continue;
        }
        match op.kind {
            ProvisionOpKind::StartService => {
                service.stop()?;
            }
            ProvisionOpKind::InstallService => {
                service.uninstall()?;
            }
            ProvisionOpKind::EnableAutostart => {
                // Reversible when ServiceManager implements disable.
                match service.disable_autostart() {
                    Ok(()) => {}
                    Err(e) => retained.push(format!("autostart may remain enabled: {e}")),
                }
            }
            ProvisionOpKind::ConfigureAgent
            | ProvisionOpKind::InitializeProject
            | ProvisionOpKind::SelfTest => {}
        }
    }
    restore_snapshots(receipt)?;
    Ok(retained)
}

fn restore_snapshots(receipt: &SetupReceipt) -> Result<()> {
    for snap in receipt.snapshots.iter().rev() {
        restore_snapshot(snap)?;
    }
    Ok(())
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
    let suite_version = suite
        .read_manifest()
        .map(|m| m.version)
        .unwrap_or_default();
    let suite_cli_hash = optional_file_sha256(std::path::Path::new(absolute_cli)).unwrap_or_default();
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
