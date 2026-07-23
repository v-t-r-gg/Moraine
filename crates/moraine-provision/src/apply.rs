//! Transactional apply of a SetupPlan.

use std::path::PathBuf;

use moraine_core::init_project;
use uuid::Uuid;

use crate::agent::adapter_for;
use crate::error::Result;
use crate::journal;
use crate::service::ServiceManager;
use crate::suite::SuitePaths;
use crate::types::{
    CompletedOperation, ProvisionOpKind, Readiness, SetupPlan, SetupReceipt,
};
use crate::verify;

/// Apply a plan: init project → configure agent → service lifecycle → self-test.
///
/// On failure mid-way, returns a receipt with `readiness = RollbackRequired` and
/// `failed_operation` set. Caller may invoke `rollback`.
pub fn apply(plan: SetupPlan, service: &dyn ServiceManager) -> Result<SetupReceipt> {
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
    // Persist journal early so crash mid-apply is recoverable.
    let _ = journal::write_journal(&receipt);

    let suite = SuitePaths::discover();
    let absolute_cli = PathBuf::from(&plan.absolute_cli);

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
                    Ok(ip) => match adapter.apply(&ip) {
                        Ok(ir) => {
                            receipt.backups.extend(ir.backups);
                            Ok(ir.actions.join("; "))
                        }
                        Err(e) => Err(e.to_string()),
                    },
                    Err(e) => Err(e.to_string()),
                }
            }
            ProvisionOpKind::InstallService => {
                match suite.absolute_service() {
                    Some(bin) => match service.install(&bin) {
                        Ok(()) => Ok(format!("installed service from {}", bin.display())),
                        Err(e) => Err(e.to_string()),
                    },
                    None => {
                        // Dev: try sibling of absolute CLI
                        let sibling = absolute_cli
                            .parent()
                            .map(|p| p.join("moraine-service"))
                            .filter(|p| p.is_file());
                        match sibling {
                            Some(bin) => match service.install(&bin) {
                                Ok(()) => Ok(format!("installed service from {}", bin.display())),
                                Err(e) => Err(e.to_string()),
                            },
                            None => Err("service binary not found in suite".into()),
                        }
                    }
                }
            }
            ProvisionOpKind::EnableAutostart => match service.enable_autostart() {
                Ok(()) => Ok("autostart enabled".into()),
                Err(e) => Err(e.to_string()),
            },
            ProvisionOpKind::StartService => match service.start() {
                Ok(()) => Ok("service started".into()),
                Err(e) => Err(e.to_string()),
            },
            ProvisionOpKind::SelfTest => {
                match verify::verify(&plan.intent) {
                    Ok(report) if report.ok => Ok(report.user_message),
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
                let _ = journal::write_journal(&receipt);
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
                receipt.error = Some(err);
                receipt.readiness = Readiness::RollbackRequired;
                let _ = journal::write_journal(&receipt);
                return Ok(receipt);
            }
        }
    }

    receipt.readiness = Readiness::Ready;
    let _ = journal::write_journal(&receipt);
    Ok(receipt)
}

/// Apply using the default platform service manager.
pub fn apply_default(plan: SetupPlan) -> Result<SetupReceipt> {
    let svc = crate::service::default_service_manager();
    apply(plan, svc.as_ref())
}

/// Restore backups and reverse reversible completed operations.
pub fn rollback(receipt: SetupReceipt, service: &dyn ServiceManager) -> Result<()> {
    // Restore file backups (newest first).
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

    // Reverse completed ops in reverse order.
    for op in receipt.completed.iter().rev() {
        if !op.success {
            continue;
        }
        match op.kind {
            ProvisionOpKind::ConfigureAgent => {
                let adapter = adapter_for(receipt.intent.agent);
                let _ = adapter.remove(&receipt.intent.project);
            }
            ProvisionOpKind::StartService => {
                let _ = service.stop();
            }
            ProvisionOpKind::InstallService => {
                let _ = service.uninstall();
            }
            ProvisionOpKind::EnableAutostart => {
                // Best-effort: stop is enough for memory manager; Linux disable is uninstall path.
            }
            ProvisionOpKind::InitializeProject => {
                // Do not delete project ledgers on rollback — data safety.
            }
            ProvisionOpKind::SelfTest => {
                // Self-test creates a run; leave it (proof of attempt).
            }
        }
    }

    let mut updated = receipt;
    updated.readiness = Readiness::Failed;
    updated.error = Some("rolled back".into());
    let _ = journal::write_journal(&updated);
    Ok(())
}

pub fn rollback_default(receipt: SetupReceipt) -> Result<()> {
    let svc = crate::service::default_service_manager();
    rollback(receipt, svc.as_ref())
}


