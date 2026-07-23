//! Shared Moraine installation-state inspection and onboarding.
//!
//! CLI and Tauri desktop both call this crate; the desktop never scrapes CLI stdout.

pub mod agent;
pub mod apply;
pub mod error;
pub mod health;
pub mod inspect;
pub mod journal;
pub mod plan;
pub mod service;
pub mod service_ready;
pub mod snapshot;
pub mod suite;
pub mod types;
pub mod verify;

pub use agent::{
    adapter_for, all_adapters, AgentAdapter, AgentDetection, BackupRecorder, CodexAdapter,
    IntegrationPlan, IntegrationReceipt, IntegrationState, IntegrationVerification,
    VecBackupRecorder,
};
pub use apply::{
    apply, apply_default, apply_receipt, apply_with_options, compute_witness, rollback,
    rollback_completed_operations, rollback_default, JournaledBackupRecorder,
};
pub use error::{ProvisionError, Result};
pub use health::{health, health_default, repair, repair_default};
pub use inspect::{detect_agent, inspect, inspect_default, inspect_suite};
pub use plan::plan;
pub use service::{
    default_service_manager, LinuxSystemdUserService, MemoryServiceManager, ServiceManager,
};
pub use service_ready::{
    default_service_probe, default_service_ready_timeout_ms, wait_for_service_ready,
    AlwaysOfflineProbe, AlwaysReadyProbe, LoopbackServiceProbe, ServiceProbe, ServiceReadyResult,
};
pub use snapshot::{durable_backup, file_sha256, restore_snapshot, snapshot_absent};
pub use suite::{
    default_http_addr, default_prefix, default_socket_path, http_get_loopback, render_systemd_unit,
    setup_transactions_dir, SuitePaths, SuiteState,
};
pub use types::FileSnapshot;
pub use types::*;
pub use verify::{
    product_capture_event_ids, verify, verify_with, verify_with_options, ControlledCapture,
    EventCapture, HookCodexCapture, VerifyOptions,
};

/// One-shot enable: plan → apply.
pub fn enable_project(intent: SetupIntent, service: &dyn ServiceManager) -> Result<ApplyOutcome> {
    let p = plan(intent, service)?;
    apply(p, service)
}

pub fn enable_project_default(intent: SetupIntent) -> Result<ApplyOutcome> {
    let svc = default_service_manager();
    enable_project(intent, svc.as_ref())
}
