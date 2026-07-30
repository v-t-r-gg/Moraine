//! Transactional setup, rollback, health & background-runtime authority.
//!
//! CLI & Tauri call this crate directly. It does not own run-domain persistence,
//! capture listener implementation or presentation. Linux runtime control is
//! isolated behind the production backend; memory implementations require
//! explicit test injection.

pub mod agent;
pub mod apply;
pub mod diagnostics;
pub mod error;
pub mod health;
pub mod inspect;
pub mod journal;
pub mod plan;
pub mod platform_support;
pub mod runtime;
/// Source compatibility for callers using the former service-manager module.
pub mod service {
    pub use crate::runtime::*;
}
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
pub use platform_support::{ensure_background_runtime_available, ensure_product_capture_supported};
pub use runtime::linux_systemd::render_systemd_unit;
#[cfg(target_os = "windows")]
pub use runtime::windows_task_scheduler::{
    registration_fingerprint, render_task_xml, WindowsTaskIdentity, WindowsTaskSchedulerRuntime,
};
pub use runtime::{
    background_runtime_manager_for_host, capture_runtime_prestate,
    default_background_runtime_manager, default_service_manager, restore_runtime_prestate,
    BackgroundRuntimeManager, LinuxSystemdUserRuntime, LinuxSystemdUserService,
    MemoryRuntimeManager, MemoryServiceManager, RuntimeInstallSpec, RuntimePrestate,
    ServiceManager, UnavailableRuntimeManager, UnsupportedRuntimeManager,
};
pub use service_ready::{
    default_service_probe, default_service_ready_timeout_ms, wait_for_service_ready,
    AlwaysOfflineProbe, AlwaysReadyProbe, LoopbackServiceProbe, ServiceProbe, ServiceReadyResult,
};
pub use snapshot::{durable_backup, file_sha256, restore_snapshot, snapshot_absent};
pub use suite::{
    capture_endpoint, default_http_addr, default_prefix, http_get_loopback, setup_transactions_dir,
    unix_capture_socket, SuitePaths, SuiteState,
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
