//! Structured provisioning types (no console text).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::suite::SuiteState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentKind {
    Codex,
}

impl AgentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentKind::Codex => "codex",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "codex" => Some(AgentKind::Codex),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Readiness {
    Ready,
    Degraded,
    Failed,
    RollbackRequired,
    NotConfigured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemState {
    pub suite: SuiteState,
    pub service: ServiceState,
    pub agents: Vec<DetectedAgent>,
    pub projects: Vec<ProjectCandidate>,
    pub readiness: Readiness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceState {
    pub installed: bool,
    pub running: bool,
    pub binary_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Product-level status, never OS jargon in the normal UI.
    pub status_message: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedAgent {
    pub kind: AgentKind,
    pub id: String,
    pub display_name: String,
    pub detected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// ReadyToConnect | NotFound | NeedsRepair | UnsupportedVersion
    pub status: String,
    pub status_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCandidate {
    pub path: String,
    pub name: String,
    pub initialized: bool,
    pub is_git: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupIntent {
    pub project: PathBuf,
    pub agent: AgentKind,
    pub enable_autostart: bool,
    /// When true, skip service install (tests / constrained environments).
    #[serde(default)]
    pub skip_service: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProvisionOpKind {
    InitializeProject,
    ConfigureAgent,
    InstallService,
    EnableAutostart,
    StartService,
    SelfTest,
}

impl ProvisionOpKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::InitializeProject => "initialize_project",
            Self::ConfigureAgent => "configure_agent",
            Self::InstallService => "install_service",
            Self::EnableAutostart => "enable_autostart",
            Self::StartService => "start_service",
            Self::SelfTest => "self_test",
        }
    }

    /// Product-level progress label (no systemd/MCP/PATH jargon).
    pub fn product_label(self) -> &'static str {
        match self {
            Self::InitializeProject => "Preparing project records",
            Self::ConfigureAgent => "Connecting coding agent",
            Self::InstallService => "Enabling background capture",
            Self::EnableAutostart => "Keeping capture available after restart",
            Self::StartService => "Starting background capture",
            Self::SelfTest => "Testing local capture",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionOperation {
    pub id: String,
    pub kind: ProvisionOpKind,
    pub product_label: String,
    pub detail: String,
    pub reversible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlan {
    pub intent: SetupIntent,
    pub operations: Vec<ProvisionOperation>,
    pub warnings: Vec<ProvisionWarning>,
    /// Absolute CLI path that will be written into agent config.
    pub absolute_cli: String,
    pub product_summary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub original_path: String,
    pub backup_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedOperation {
    pub id: String,
    pub kind: ProvisionOpKind,
    pub product_label: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupReceipt {
    pub transaction_id: Uuid,
    pub intent: SetupIntent,
    pub completed: Vec<CompletedOperation>,
    pub backups: Vec<BackupRecord>,
    pub readiness: Readiness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub journal_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationStep {
    pub id: String,
    pub product_label: String,
    pub passed: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationReport {
    pub ok: bool,
    pub readiness: Readiness,
    pub steps: Vec<VerificationStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    pub user_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthStatus {
    Pass,
    Warn,
    Fail,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairAction {
    pub id: String,
    /// Product label for the Fix button.
    pub label: String,
    pub kind: RepairKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepairKind {
    StartService,
    InstallService,
    InitProject,
    RepairAgentIntegration,
    RestartService,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub id: String,
    pub status: HealthStatus,
    pub user_message: String,
    pub technical_detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair: Option<RepairAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub ok: bool,
    pub checks: Vec<HealthCheck>,
    pub readiness: Readiness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairResult {
    pub ok: bool,
    pub action_id: String,
    pub user_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLog {
    pub line: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}
