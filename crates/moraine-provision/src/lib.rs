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
pub mod suite;
pub mod types;
pub mod verify;

pub use agent::{
    adapter_for, all_adapters, AgentAdapter, AgentDetection, CodexAdapter, IntegrationPlan,
    IntegrationReceipt, IntegrationState, IntegrationVerification,
};
pub use apply::{apply, apply_default, rollback, rollback_default};
pub use error::{ProvisionError, Result};
pub use health::{health, health_default, repair, repair_default};
pub use inspect::{detect_agent, inspect, inspect_default, inspect_suite};
pub use plan::plan;
pub use service::{
    default_service_manager, LinuxSystemdUserService, MemoryServiceManager, ServiceManager,
};
pub use suite::{
    default_http_addr, default_prefix, default_socket_path, http_get_loopback, render_systemd_unit,
    setup_transactions_dir, SuitePaths, SuiteState,
};
pub use types::*;
pub use verify::verify;

/// One-shot enable: plan → apply → return receipt (Ready only after self-test).
pub fn enable_project(
    intent: SetupIntent,
    service: &dyn ServiceManager,
) -> Result<SetupReceipt> {
    let p = plan(intent, service)?;
    apply(p, service)
}

pub fn enable_project_default(intent: SetupIntent) -> Result<SetupReceipt> {
    let svc = default_service_manager();
    enable_project(intent, svc.as_ref())
}
