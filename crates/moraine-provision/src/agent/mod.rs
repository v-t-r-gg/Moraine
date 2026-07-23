//! Agent integration adapters (detect / plan / apply / verify / remove).

mod codex;

pub use codex::CodexAdapter;

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

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
    }
}

pub fn all_adapters() -> Vec<Arc<dyn AgentAdapter>> {
    vec![Arc::new(CodexAdapter::new())]
}
