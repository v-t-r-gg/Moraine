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
    /// Product path: background capture + adapter event verified.
    Ready,
    Degraded,
    Failed,
    RollbackRequired,
    NotConfigured,
    /// Dev/test path only (`skip_service` / DirectCoreTest) — not product Ready.
    DirectVerified,
}

/// Product capture vs direct core test (never conflate Ready).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum VerificationMode {
    /// Requires Codex, MCP+hooks, service health, successful hook delivery, discoverable run.
    #[default]
    ProductCapture,
    /// Explicit test/dev path using core APIs; yields DirectVerified, never product Ready.
    DirectCoreTest,
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
    /// True when a service **registration** exists (unit/task), not merely a binary on disk.
    pub installed: bool,
    /// Suite service binary is present.
    pub binary_present: bool,
    /// OS registration (systemd unit / equivalent) is present.
    #[serde(default)]
    pub registration_present: bool,
    /// Registration appears valid (unit exists and references a present binary when known).
    #[serde(default)]
    pub registration_valid: bool,
    pub running: bool,
    /// Loopback endpoint answered (when probed).
    #[serde(default)]
    pub endpoint_ready: bool,
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

/// Snapshot of system state at plan time; apply rejects if witness drifts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetupStateWitness {
    pub project: String,
    pub absolute_cli: String,
    /// Suite product version (when known).
    #[serde(default)]
    pub suite_version: String,
    /// SHA-256 of suite CLI bytes when available.
    #[serde(default)]
    pub suite_cli_hash: String,
    /// Hash of project `.codex/config.toml` if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_config_hash: Option<String>,
    /// Hash of project `.codex/hooks.json` if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_hooks_hash: Option<String>,
    /// Hash of service unit file if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_unit_hash: Option<String>,
    pub project_initialized: bool,
    pub service_installed: bool,
    #[serde(default)]
    pub service_registration_valid: bool,
    pub service_running: bool,
    pub enable_autostart: bool,
    pub skip_service: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlan {
    /// Stable id for the approved plan (wizard must apply this plan, not recompute).
    pub plan_id: Uuid,
    pub intent: SetupIntent,
    pub operations: Vec<ProvisionOperation>,
    pub warnings: Vec<ProvisionWarning>,
    /// Absolute CLI path that will be written into agent config.
    pub absolute_cli: String,
    pub product_summary: Vec<String>,
    pub state_witness: SetupStateWitness,
}

/// Result of transactional apply (auto-rollback attempted on failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum ApplyOutcome {
    Ready { receipt: SetupReceipt },
    /// Dev/test self-test path completed without product Ready.
    DirectVerified { receipt: SetupReceipt },
    RolledBack {
        receipt: SetupReceipt,
        original_error: String,
    },
    RollbackRequired {
        receipt: SetupReceipt,
        original_error: String,
        rollback_error: String,
    },
}

impl ApplyOutcome {
    pub fn receipt(&self) -> &SetupReceipt {
        match self {
            Self::Ready { receipt }
            | Self::DirectVerified { receipt }
            | Self::RolledBack { receipt, .. }
            | Self::RollbackRequired { receipt, .. } => receipt,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Ready { .. } | Self::DirectVerified { .. })
    }
}

/// Pre-mutation file snapshot for write-ahead recovery.
///
/// `Absent` means the path did not exist before the transaction — rollback deletes it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FileSnapshot {
    Existing {
        path: String,
        backup_path: String,
        original_hash: String,
        created_at: String,
    },
    Absent {
        path: String,
        created_at: String,
    },
}

impl FileSnapshot {
    pub fn path(&self) -> &str {
        match self {
            Self::Existing { path, .. } | Self::Absent { path, .. } => path,
        }
    }
}

/// Backward-compatible alias used by older call sites / receipts.
pub type BackupRecord = FileSnapshot;

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
    /// File mutations recorded before apply (existing backups + previously-absent paths).
    #[serde(alias = "backups")]
    pub snapshots: Vec<FileSnapshot>,
    pub readiness: Readiness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// True when rollback left known non-reversible changes (e.g. autostart enabled).
    #[serde(default)]
    pub retained_changes: Vec<String>,
    pub journal_path: String,
}

impl SetupReceipt {
    /// Compatibility accessor.
    pub fn backups(&self) -> &[FileSnapshot] {
        &self.snapshots
    }
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
