//! Agent integration adapters (detect / plan / apply / verify / remove).

mod claude_code;
mod codex;

pub use claude_code::ClaudeCodeAdapter;
pub use codex::CodexAdapter;

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use moraine_core::CaptureCapabilityProfile;

use crate::error::Result;
use crate::types::{AgentKind, FileSnapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetection {
    pub kind: AgentKind,
    pub detected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub status: String,
    pub status_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationState {
    /// Fully configured only when both MCP and hooks are present.
    pub configured: bool,
    pub mcp_present: bool,
    pub hooks_present: bool,
    pub absolute_cli: Option<String>,
    pub config_path: Option<String>,
    pub details: Vec<String>,
    pub needs_repair: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationPlan {
    pub kind: AgentKind,
    pub project: String,
    pub absolute_cli: String,
    pub actions: Vec<String>,
    pub product_labels: Vec<String>,
    pub files_to_touch: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationReceipt {
    pub kind: AgentKind,
    pub project: String,
    pub absolute_cli: String,
    pub actions: Vec<String>,
    pub snapshots: Vec<FileSnapshot>,
    pub config_path: Option<String>,
    pub hooks_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationVerification {
    pub ok: bool,
    pub absolute_cli_ok: bool,
    pub config_present: bool,
    pub mcp_present: bool,
    pub hooks_present: bool,
    pub messages: Vec<String>,
}

/// Records a file snapshot **before** the corresponding mutation; implementations
/// must persist (journal + fsync) before returning Ok.
pub trait BackupRecorder: Send {
    fn record_snapshot(&mut self, snapshot: FileSnapshot) -> Result<()>;
}

/// In-memory sink (tests only). Production apply uses journaled recorder.
pub struct VecBackupRecorder {
    pub snapshots: Vec<FileSnapshot>,
}

impl VecBackupRecorder {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    pub fn backups(&self) -> &[FileSnapshot] {
        &self.snapshots
    }
}

impl Default for VecBackupRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl BackupRecorder for VecBackupRecorder {
    fn record_snapshot(&mut self, snapshot: FileSnapshot) -> Result<()> {
        self.snapshots.push(snapshot);
        Ok(())
    }
}

pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn kind(&self) -> AgentKind;

    /// Generic mechanical capture capabilities for this adapter (fidelity reporting).
    fn capture_capabilities(&self) -> CaptureCapabilityProfile;

    fn detect(&self) -> Result<AgentDetection>;
    fn inspect(&self, project: &Path) -> Result<IntegrationState>;
    fn plan_install(&self, project: &Path, absolute_cli: &Path) -> Result<IntegrationPlan>;
    /// Apply integration. Each backup is recorded via `recorder` *before* mutation.
    fn apply(
        &self,
        plan: &IntegrationPlan,
        recorder: &mut dyn BackupRecorder,
    ) -> Result<IntegrationReceipt>;
    fn verify(&self, project: &Path, expected_cli: &Path) -> Result<IntegrationVerification>;
    fn remove(&self, project: &Path) -> Result<Vec<FileSnapshot>>;
}

pub fn adapter_for(kind: AgentKind) -> Arc<dyn AgentAdapter> {
    match kind {
        AgentKind::Codex => Arc::new(CodexAdapter::new()),
        AgentKind::ClaudeCode => Arc::new(ClaudeCodeAdapter::new()),
    }
}

pub fn all_adapters() -> Vec<Arc<dyn AgentAdapter>> {
    vec![
        Arc::new(CodexAdapter::new()),
        Arc::new(ClaudeCodeAdapter::new()),
    ]
}

/// Authoritative built-in capability table. Unknown IDs resolve to all-`Unknown`.
pub fn capability_profile_for_integration(integration: &str) -> CaptureCapabilityProfile {
    let id = integration.trim();
    if id.is_empty() || id == "unknown" {
        return CaptureCapabilityProfile::unknown();
    }
    for adapter in all_adapters() {
        if adapter.id() == id {
            return adapter.capture_capabilities();
        }
    }
    CaptureCapabilityProfile::unknown()
}

/// Resolve the capability profile for a run from durable session integration
/// (application boundary). Propagates bound-session validation errors.
pub fn capability_profile_for_run(
    project: Option<&Path>,
    run_id: uuid::Uuid,
) -> moraine_core::Result<CaptureCapabilityProfile> {
    let integration = moraine_core::peek_run_integration(project, run_id)?;
    Ok(capability_profile_for_integration(
        integration.as_deref().unwrap_or(""),
    ))
}

/// Capture fidelity report with the authoritative adapter capability profile.
pub fn capture_fidelity_report_for_run(
    project: Option<&Path>,
    run_id: uuid::Uuid,
) -> moraine_core::Result<moraine_core::CaptureFidelityReport> {
    let profile = capability_profile_for_run(project, run_id)?;
    moraine_core::capture_fidelity_report(project, run_id, &profile)
}

#[cfg(test)]
mod capability_tests {
    use super::*;
    use moraine_core::CapabilitySupport;

    #[test]
    fn codex_profile_tools_supported() {
        let p = capability_profile_for_integration("codex");
        assert_eq!(p.integration_id, "codex");
        assert_eq!(p.session_lifecycle, CapabilitySupport::Supported);
        assert_eq!(p.prompt_activity, CapabilitySupport::Supported);
        assert_eq!(p.tool_activity, CapabilitySupport::Supported);
        assert_eq!(p.semantic_protocol, CapabilitySupport::Supported);
    }

    #[test]
    fn claude_code_tool_capability_not_supported() {
        let p = capability_profile_for_integration("claude-code");
        assert_eq!(p.integration_id, "claude-code");
        assert_eq!(p.tool_activity, CapabilitySupport::NotSupported);
        assert_eq!(p.session_lifecycle, CapabilitySupport::Supported);
        assert_eq!(p.semantic_protocol, CapabilitySupport::Supported);
    }

    #[test]
    fn unknown_integration_uses_unknown_profile() {
        let p = capability_profile_for_integration("some-future-agent");
        assert_eq!(p.integration_id, "unknown");
        assert_eq!(p.session_lifecycle, CapabilitySupport::Unknown);
        assert_eq!(p.prompt_activity, CapabilitySupport::Unknown);
        assert_eq!(p.tool_activity, CapabilitySupport::Unknown);
        assert_eq!(p.semantic_protocol, CapabilitySupport::Unknown);
        let empty = capability_profile_for_integration("");
        assert_eq!(empty, CaptureCapabilityProfile::unknown());
    }

    #[test]
    fn adapter_trait_matches_table() {
        let codex = adapter_for(AgentKind::Codex).capture_capabilities();
        assert_eq!(codex, capability_profile_for_integration("codex"));
        let claude = adapter_for(AgentKind::ClaudeCode).capture_capabilities();
        assert_eq!(claude, capability_profile_for_integration("claude-code"));
    }
}
