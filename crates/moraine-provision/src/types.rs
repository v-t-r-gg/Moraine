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
    pub platform: moraine_platform::PlatformCapabilities,
    pub suite: SuiteState,
    pub service: ServiceState,
    pub agents: Vec<DetectedAgent>,
    pub projects: Vec<ProjectCandidate>,
    pub readiness: Readiness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundRuntimeState {
    #[serde(default)]
    pub backend: BackgroundRuntimeBackend,
    #[serde(default = "default_true")]
    pub supported: bool,
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
    /// Whether the service is set to start at login / user session.
    #[serde(default)]
    pub autostart_enabled: bool,
    /// Loopback endpoint answered (when probed).
    #[serde(default)]
    pub endpoint_ready: bool,
    #[serde(default)]
    pub diagnostics_ready: bool,
    #[serde(default)]
    pub capture_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Platform runtime result code when the backend exposes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_result: Option<i32>,
    /// Product-level status, never OS jargon in the normal UI.
    pub status_message: String,
    pub platform: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration: Option<RuntimeRegistrationState>,
}

fn default_true() -> bool {
    true
}

pub type ServiceState = BackgroundRuntimeState;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundRuntimeBackend {
    LinuxSystemdUser,
    WindowsTaskScheduler,
    Unsupported,
    #[default]
    MemoryTest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRegistrationKind {
    SystemdUserUnit,
    WindowsTaskSchedulerTask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRegistrationState {
    pub kind: RuntimeRegistrationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
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
    #[serde(default)]
    pub integration_configured: bool,
    #[serde(default)]
    pub integration_needs_repair: bool,
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
    RegisterProject,
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
            Self::RegisterProject => "register_project",
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
            Self::RegisterProject => "Remembering project",
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

impl SetupPlan {
    /// Whether applying this serialized plan requires the ProductCapture platform backend.
    ///
    /// Plans cross process boundaries, so this deliberately considers both the
    /// declared intent and the operations actually present in the plan.
    pub fn requires_product_capture(&self) -> bool {
        !self.intent.skip_service || self.has_background_runtime_operations()
    }

    pub fn has_background_runtime_operations(&self) -> bool {
        self.operations.iter().any(|operation| {
            matches!(
                operation.kind,
                ProvisionOpKind::InstallService
                    | ProvisionOpKind::EnableAutostart
                    | ProvisionOpKind::StartService
            )
        })
    }
}

/// Result of transactional apply (auto-rollback attempted on failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "outcome"
)]
pub enum ApplyOutcome {
    Ready {
        receipt: SetupReceipt,
    },
    /// Dev/test self-test path completed without product Ready.
    DirectVerified {
        receipt: SetupReceipt,
    },
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
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind",
    deny_unknown_fields
)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowsTaskSnapshot {
    pub task_path: String,
    pub captured_at: String,
    pub state: WindowsTaskSnapshotState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WindowsTaskSnapshotState {
    Existing {
        xml: String,
        security_descriptor: String,
        fingerprint: String,
    },
    Absent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RuntimeRegistrationSnapshot {
    File(FileSnapshot),
    WindowsTask(WindowsTaskSnapshot),
}

impl RuntimeRegistrationSnapshot {
    pub fn path(&self) -> &str {
        match self {
            Self::File(snapshot) => snapshot.path(),
            Self::WindowsTask(snapshot) => &snapshot.task_path,
        }
    }
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

/// Captured before first service mutation so rollback restores exact prestate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSnapshot {
    pub registration: RuntimeRegistrationSnapshot,
    pub was_running: bool,
    pub autostart_was_enabled: bool,
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
    /// Service prestate captured on first Install/Start/EnableAutostart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_prestate: Option<ServiceSnapshot>,
    /// True once an enable-autostart mutation attempt was durably journaled.
    /// The side effect may have completed even when the manager returned an error.
    #[serde(default)]
    pub transaction_enabled_autostart: bool,
    /// True once a start mutation attempt for a previously-stopped service was
    /// durably journaled. Rollback must stop it even when `start` returned an error.
    #[serde(default)]
    pub transaction_started_service: bool,
    /// True once an install mutation attempt was durably journaled. The unit may
    /// have been written even when `install` returned an error.
    #[serde(default)]
    pub transaction_wrote_unit: bool,
    /// True when this transaction created project-local `.moraine/` state.
    /// Rollback intentionally retains ledgers rather than deleting user records.
    #[serde(default)]
    pub transaction_initialized_project: bool,
    /// True when this transaction added the root to the rebuildable project registry.
    #[serde(default)]
    pub transaction_registered_project: bool,
    pub readiness: Readiness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Notes when rollback could not fully restore (should stay empty on clean paths).
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

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn c3_service_snapshot_registration_shape_remains_readable() {
        let legacy = serde_json::json!({
            "registration": {
                "kind": "absent",
                "path": "/tmp/moraine-service.service",
                "createdAt": "2026-01-01T00:00:00Z"
            },
            "wasRunning": false,
            "autostartWasEnabled": true
        });
        let snapshot: ServiceSnapshot = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(snapshot.registration.path(), "/tmp/moraine-service.service");
        assert_eq!(serde_json::to_value(snapshot).unwrap(), legacy);
    }

    #[test]
    fn windows_task_snapshots_round_trip_without_changing_file_snapshots() {
        let existing = RuntimeRegistrationSnapshot::WindowsTask(WindowsTaskSnapshot {
            task_path: r"\Moraine Background Capture (abc123def456)".into(),
            captured_at: "2026-07-29T00:00:00Z".into(),
            state: WindowsTaskSnapshotState::Existing {
                xml: "<Task />".into(),
                security_descriptor: "O:SYD:P(A;;FA;;;SY)".into(),
                fingerprint: "abc".into(),
            },
        });
        let absent = RuntimeRegistrationSnapshot::WindowsTask(WindowsTaskSnapshot {
            task_path: r"\Moraine Background Capture (abc123def456)".into(),
            captured_at: "2026-07-29T00:00:00Z".into(),
            state: WindowsTaskSnapshotState::Absent,
        });
        for snapshot in [existing, absent] {
            let value = serde_json::to_value(&snapshot).unwrap();
            assert_eq!(
                serde_json::from_value::<RuntimeRegistrationSnapshot>(value).unwrap(),
                snapshot
            );
            assert!(snapshot.path().starts_with(r"\Moraine Background Capture"));
        }

        let legacy_file = serde_json::json!({
            "kind": "absent",
            "path": "/tmp/moraine-service.service",
            "createdAt": "2026-01-01T00:00:00Z"
        });
        let parsed: RuntimeRegistrationSnapshot =
            serde_json::from_value(legacy_file.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), legacy_file);
    }

    #[test]
    fn malformed_or_ambiguous_registration_snapshots_fail() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"taskPath": "\\Moraine"}),
            serde_json::json!({
                "kind": "absent",
                "path": "/tmp/unit",
                "createdAt": "now",
                "taskPath": "\\Moraine",
                "capturedAt": "now",
                "state": {"kind": "absent"}
            }),
        ] {
            assert!(serde_json::from_value::<RuntimeRegistrationSnapshot>(value).is_err());
        }
    }

    #[test]
    fn shared_platform_fixture_covers_runtime_state_contract() {
        let raw = include_str!("../../../src/shared/api/platform.contract.fixture.json");
        let values: Vec<serde_json::Value> = serde_json::from_str(raw).unwrap();
        let states: Vec<BackgroundRuntimeState> = values
            .into_iter()
            .map(|value| serde_json::from_value(value["runtime"].clone()).unwrap())
            .collect();
        assert_eq!(
            states[0].backend,
            BackgroundRuntimeBackend::LinuxSystemdUser
        );
        assert!(states[0].capture_ready);
        assert_eq!(states[1].backend, BackgroundRuntimeBackend::Unsupported);
        assert!(!states[1].supported);
    }
}
