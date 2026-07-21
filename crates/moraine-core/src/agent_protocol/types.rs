use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::git::GitContextSummary;

pub const MAX_SUMMARY_CHARS: usize = 2_000;
pub const MAX_FIELD_CHARS: usize = 4_000;
pub const MAX_CHECKPOINT_ITEMS: usize = 50;
pub const MAX_RECENT_CHECKPOINTS_IN_SHOW: usize = 5;
pub const MAX_RECENT_LIST_IN_SHOW: usize = 5;
/// Compact lifetime idempotency index; never silently drops keys.
/// Detail history may still be capped separately.
pub const MAX_IDEMPOTENCY_INDEX: usize = 10_000;

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

/// How completely Moraine captured this run (independent of lifecycle).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaptureCoverage {
    Full,
    MechanicalOnly,
    SemanticOnly,
    Partial,
    #[default]
    Unknown,
}

impl CaptureCoverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::MechanicalOnly => "mechanical_only",
            Self::SemanticOnly => "semantic_only",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
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
    ToolResult,
    Artifact,
    Path,
    Url,
    Note,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandResult => "command_result",
            Self::ToolResult => "tool_result",
            Self::Artifact => "artifact",
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
    /// Moraine observed only that an invocation was requested/begun.
    InvocationObserved,
    /// Moraine observed a result payload directly.
    ResultObserved,
    /// Captured mechanically by Moraine (e.g. verified execution result or Git context).
    MoraineCaptured,
    /// External system or URL reference.
    ExternalReference,
}

impl EvidenceProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentReported => "agent_reported",
            Self::InvocationObserved => "invocation_observed",
            Self::ResultObserved => "result_observed",
            Self::MoraineCaptured => "moraine_captured",
            Self::ExternalReference => "external_reference",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSummary {
    pub evidence_id: Uuid,
    pub kind: EvidenceKind,
    pub provenance: EvidenceProvenance,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub summary: String,
    pub occurred_at: DateTime<Utc>,
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

/// Compact lifetime idempotency record (safe to retain indefinitely).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IdempotencyRecord {
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
    /// Expected Markdown hash after successful apply.
    pub expected_content_hash: String,
    pub phase: IncompletePhase,
    pub created_at: DateTime<Utc>,
    /// Next committed agent state if Markdown reaches `expected_content_hash`.
    /// Must not itself contain a nested incomplete_op.
    pub pending_agent: Box<AgentRunState>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncompletePhase {
    /// Sidecar intent recorded; Markdown not yet replaced.
    Begun,
    /// Markdown written; promotion of pending_agent still required.
    MarkdownApplied,
}

/// Committed agent-run protocol state.
///
/// Authority model A: Moraine-managed Markdown regions are projections of this
/// state. Only `## Human notes` is free-form human text that survives agent
/// mutations. Human edits outside that region are not preserved on the next
/// protocol operation.
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
    /// Lifetime compact idempotency index (key → record). Not silently expired.
    #[serde(default)]
    pub idempotency: std::collections::BTreeMap<String, IdempotencyRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incomplete_op: Option<Box<IncompleteOp>>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
    /// Capture channel honesty (hooks / MCP / both / unknown).
    #[serde(default)]
    pub capture_coverage: CaptureCoverage,
    /// Agent-host session this run is bound to, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// True until a semantic `run_start` confirms this mechanically created run.
    #[serde(default)]
    pub provisional: bool,
    /// Mechanically captured evidence summaries attached to this run.
    #[serde(default)]
    pub evidence: Vec<EvidenceSummary>,
}

impl AgentRunState {
    pub fn find_idempotency(&self, key: &str) -> Option<&IdempotencyRecord> {
        self.idempotency.get(key)
    }

    /// True when a *new* key can be inserted without exceeding the hard ceiling.
    pub fn has_idempotency_capacity_for(&self, key: &str) -> bool {
        self.idempotency.contains_key(key) || self.idempotency.len() < MAX_IDEMPOTENCY_INDEX
    }

    pub fn record_idempotency(
        &mut self,
        key: String,
        rec: IdempotencyRecord,
    ) -> Result<(), crate::error::Error> {
        if !self.has_idempotency_capacity_for(&key) {
            return Err(crate::error::Error::IdempotencyIndexFull {
                max: MAX_IDEMPOTENCY_INDEX,
            });
        }
        self.idempotency.insert(key, rec);
        Ok(())
    }

    pub fn bump_revision(&mut self) -> Result<(), crate::error::Error> {
        self.record_revision = self
            .record_revision
            .checked_add(1)
            .ok_or_else(|| crate::error::Error::other("agent record_revision overflow"))?;
        Ok(())
    }
}
