//! Write-ahead transactional apply with automatic rollback.

use std::path::{Path, PathBuf};

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
            ProvisionOpKind::InitializeProject => match init_project(Some(&plan.intent.project)) {
                Ok(r) => Ok(format!(
                    "project ready at {} (created={})",
                    r.project_root.display(),
                    r.created
                )),
                Err(e) => Err(e.to_string()),
            },
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
                capture_service_prestate(&mut receipt, service, &suite)?;
                let bin = suite.absolute_service().or_else(|| {
                    absolute_cli
                        .parent()
                        .map(|p| p.join("moraine-service"))
                        .filter(|p| p.is_file())
                });
                match bin {
                    Some(bin) => {
                        // Snapshot existing unit before overwrite (also in service_prestate).
                        if suite.unit.is_file() {
                            let snap = durable_backup(&suite.unit)?;
                            receipt.snapshots.push(snap);
                            journal::write_journal(&receipt)?;
                        } else {
                            let snap = snapshot_absent(&suite.unit);
                            receipt.snapshots.push(snap.clone());
                            // Also ensure prestate registration is Absent
                            journal::write_journal(&receipt)?;
                        }
                        match service.install(&bin) {
                            Ok(()) => {
                                receipt.transaction_wrote_unit = true;
                                Ok(format!("installed service from {}", bin.display()))
                            }
                            Err(e) => Err(e.to_string()),
                        }
                    }
                    None => Err("service binary not found in suite".into()),
                }
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
                    match service.enable_autostart() {
                        Ok(()) => {
                            receipt.transaction_enabled_autostart = true;
                            Ok("autostart enabled".into())
                        }
                        Err(e) => Err(e.to_string()),
                    }
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
                    match service.start() {
                        Ok(()) => {
                            let ready = probe.wait_ready(
                                crate::service_ready::default_service_ready_timeout_ms(),
                            );
                            if ready.ready
                                || service
                                    .inspect()
                                    .map(|s| s.platform == "memory" && s.running)
                                    .unwrap_or(false)
                            {
                                if !was_running {
                                    receipt.transaction_started_service = true;
                                }
                                Ok(format!("service started ({})", ready.message))
                            } else {
                                Err(ready.message)
                            }
                        }
                        Err(e) => Err(e.to_string()),
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

fn capture_service_prestate(
    receipt: &mut SetupReceipt,
    service: &dyn ServiceManager,
    suite: &SuitePaths,
) -> Result<()> {
    if receipt.service_prestate.is_some() {
        return Ok(());
    }
    let st = service.inspect()?;
    let registration = if suite.unit.is_file() {
        durable_backup(&suite.unit)?
    } else if let Some(ref up) = st.unit_path {
        let p = Path::new(up);
        if p.is_file() {
            durable_backup(p)?
        } else {
            snapshot_absent(p)
        }
    } else {
        snapshot_absent(&suite.unit)
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
    let retained = Vec::new();
    let pre = receipt.service_prestate.as_ref();

    // Reverse process lifecycle first (newest completed ops).
    for op in receipt.completed.iter().rev() {
        if !op.success {
            continue;
        }
        match op.kind {
            ProvisionOpKind::StartService => {
                // Stop only if this transaction started a previously-stopped service.
                if receipt.transaction_started_service {
                    service.stop()?;
                }
                // If unit was repaired while previously running, ensure it is running again.
                if pre.map(|p| p.was_running).unwrap_or(false)
                    && receipt.transaction_wrote_unit
                    && !receipt.transaction_started_service
                {
                    let _ = service.start();
                }
            }
            ProvisionOpKind::InstallService => {
                // Restore prior unit rather than unconditional uninstall.
                if let Some(pre) = pre {
                    match &pre.registration {
                        FileSnapshot::Existing { .. } => {
                            restore_snapshot(&pre.registration)?;
                            // Reload after restore when Linux.
                            let _ = service.stop();
                            if pre.was_running {
                                let _ = service.start();
                            }
                        }
                        FileSnapshot::Absent { .. } => {
                            // No prior unit — uninstall what we created.
                            service.uninstall()?;
                        }
                    }
                } else if receipt.transaction_wrote_unit {
                    service.uninstall()?;
                }
            }
            ProvisionOpKind::EnableAutostart => {
                // Disable only when this transaction enabled it.
                if receipt.transaction_enabled_autostart {
                    service.disable_autostart()?;
                }
            }
            ProvisionOpKind::ConfigureAgent
            | ProvisionOpKind::InitializeProject
            | ProvisionOpKind::SelfTest => {}
        }
    }

    // Restore project file snapshots (Codex etc.) — not the unit if already restored above.
    for snap in receipt.snapshots.iter().rev() {
        // Skip unit path if we already restored via service prestate Existing.
        if let Some(pre) = pre {
            if snap.path() == pre.registration.path()
                && matches!(pre.registration, FileSnapshot::Existing { .. })
            {
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
