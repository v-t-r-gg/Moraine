use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::git::GitContextSummary;

pub const MAX_SUMMARY_CHARS: usize = 2_000;
pub const MAX_FIELD_CHARS: usize = 4_000;
pub const MAX_CHECKPOINT_ITEMS: usize = 50;
pub const MAX_RECENT_CHECKPOINTS_IN_SHOW: usize = 5;
pub const MAX_COMPLETED_OPS_RETAINED: usize = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunLifecycle {
    Active,
    ReadyForReview,
}

impl RunLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::ReadyForReview => "ready_for_review",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RationalItem {
    pub choice: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    CommandResult,
    Path,
    Url,
    Note,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandResult => "command_result",
            Self::Path => "path",
            Self::Url => "url",
            Self::Note => "note",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProvenance {
    /// Agent asserted this result; Moraine did not capture or verify it.
    #[default]
    AgentReported,
    /// Captured mechanically by Moraine (e.g. Git context).
    MoraineCaptured,
}

impl EvidenceProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentReported => "agent_reported",
            Self::MoraineCaptured => "moraine_captured",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub kind: EvidenceKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub provenance: EvidenceProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointRecord {
    pub op_id: Uuid,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationales: Vec<RationalItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_questions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitContextSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleEvent {
    pub op_id: Uuid,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
    /// ready | resume
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitContextSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompletedOp {
    pub idempotency_key: String,
    /// Logical payload fingerprint (SHA-256 hex of canonical JSON).
    pub payload_hash: String,
    pub op_id: Uuid,
    pub kind: String,
    pub content_hash: String,
    pub record_revision: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IncompleteOp {
    pub op_id: Uuid,
    pub idempotency_key: String,
    pub kind: String,
    pub payload_hash: String,
    /// Hash of Markdown before this op began.
    pub base_content_hash: String,
    /// Expected Markdown hash after successful apply (when known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_content_hash: Option<String>,
    pub phase: IncompletePhase,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncompletePhase {
    /// Sidecar marked; Markdown not yet replaced.
    Begun,
    /// Markdown written; sidecar finalization pending.
    MarkdownApplied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunState {
    pub lifecycle: RunLifecycle,
    pub record_revision: u64,
    pub objective: String,
    /// Path relative to project root, using `/` separators.
    pub record_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    pub start_idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starting_git: Option<GitContextSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_git: Option<GitContextSummary>,
    #[serde(default)]
    pub checkpoints: Vec<CheckpointRecord>,
    #[serde(default)]
    pub lifecycle_events: Vec<LifecycleEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_summary: Option<String>,
    #[serde(default)]
    pub completed_ops: Vec<CompletedOp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incomplete_op: Option<IncompleteOp>,
    /// Aggregated open risks (latest union; compact for show).
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

impl AgentRunState {
    pub fn find_completed_op(&self, key: &str) -> Option<&CompletedOp> {
        self.completed_ops
            .iter()
            .rev()
            .find(|o| o.idempotency_key == key)
    }

    pub fn push_completed(&mut self, op: CompletedOp) {
        self.completed_ops.push(op);
        if self.completed_ops.len() > MAX_COMPLETED_OPS_RETAINED {
            let drop_n = self.completed_ops.len() - MAX_COMPLETED_OPS_RETAINED;
            self.completed_ops.drain(0..drop_n);
        }
    }
}
