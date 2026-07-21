//! Ordinary (non-forensic) projections for redacted ledger content.
//!
//! Canonical sidecars may retain prior claim text for integrity. Every ordinary
//! DTO path (CLI/MCP/Tauri/desktop/service discovery) must go through these
//! helpers so redacted content is never reconstructed ad hoc in a transport.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::append_ops::{current_checkpoint_claim, is_redacted};
use super::types::{AgentRunState, AppendOnlyOpRecord, CheckpointRecord, LedgerRelationship};

/// Ordinary-view marker for withheld claim text.
pub const REDACTED_MARKER: &str = "[REDACTED]";

/// Sentinel used in C1 complete-payload nonleak tests (must never appear in ordinary JSON).
#[cfg(test)]
pub const REDACTION_TEST_SENTINEL: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91";

/// Compact ordinary projection of a checkpoint for selectors / recent lists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrdinaryCheckpointSummary {
    pub op_id: Uuid,
    pub created_at: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub redacted: bool,
}

/// Project a single checkpoint's ordinary summary line.
pub fn project_checkpoint_summary(
    agent: &AgentRunState,
    cp: &CheckpointRecord,
) -> OrdinaryCheckpointSummary {
    let redacted = is_redacted(agent, cp.op_id);
    OrdinaryCheckpointSummary {
        op_id: cp.op_id,
        created_at: cp.created_at.to_rfc3339(),
        summary: if redacted {
            REDACTED_MARKER.into()
        } else {
            truncate(&current_checkpoint_claim(agent, cp.op_id), 240)
        },
        redacted,
    }
}

/// Ordinary projection of an append-only op (withholds prior/new when redacted).
pub fn project_append_only_op(
    agent: &AgentRunState,
    op: &AppendOnlyOpRecord,
) -> AppendOnlyOpRecord {
    let mut out = op.clone();
    let target_redacted = op
        .target_id
        .map(|id| is_redacted(agent, id))
        .unwrap_or(false);

    match op.relationship {
        LedgerRelationship::Redacted => {
            // Never return prior claim text on redaction ops.
            out.previous_content = None;
            out.new_content = Some(REDACTED_MARKER.into());
        }
        LedgerRelationship::Amended | LedgerRelationship::Superseded if target_redacted => {
            out.previous_content = None;
            out.new_content = Some(REDACTED_MARKER.into());
        }
        LedgerRelationship::Observation => {
            // Observations are independent human text; leave as-is.
        }
        _ => {}
    }
    out
}

/// Project all append-only ops for ordinary list APIs.
pub fn project_append_only_ops(agent: &AgentRunState) -> Vec<AppendOnlyOpRecord> {
    agent
        .append_only_ops
        .iter()
        .map(|op| project_append_only_op(agent, op))
        .collect()
}

/// Whether a risk/question string is content from a redacted checkpoint (for agent-level lists).
pub fn is_redacted_checkpoint_derived_text(agent: &AgentRunState, text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    for cp in &agent.checkpoints {
        if !is_redacted(agent, cp.op_id) {
            continue;
        }
        if cp.risks.iter().any(|r| r.trim() == t) {
            return true;
        }
        if cp.open_questions.iter().any(|q| q.trim() == t) {
            return true;
        }
        if cp.actions.iter().any(|a| a.trim() == t) {
            return true;
        }
        if cp.summary.trim() == t {
            return true;
        }
        for op in &agent.append_only_ops {
            if op.target_id != Some(cp.op_id) {
                continue;
            }
            if op.previous_content.as_deref().map(str::trim) == Some(t) {
                return true;
            }
            if op.new_content.as_deref().map(str::trim) == Some(t)
                && op.new_content.as_deref() != Some(REDACTED_MARKER)
            {
                return true;
            }
        }
    }
    false
}

/// Filter agent-level risk/question lists for ordinary show/discovery.
pub fn project_string_list_without_redacted_claims(
    agent: &AgentRunState,
    items: &[String],
) -> Vec<String> {
    items
        .iter()
        .filter(|s| !is_redacted_checkpoint_derived_text(agent, s))
        .cloned()
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let head: String = t.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// Assert helper: complete JSON must not contain any of the needles.
#[cfg(test)]
pub fn assert_json_omits(value: &impl Serialize, needles: &[&str]) {
    let s = serde_json::to_string(value).expect("serialize");
    for n in needles {
        assert!(
            !s.contains(n),
            "ordinary JSON leaked {n:?}: {}",
            &s[..s.len().min(800)]
        );
    }
}
