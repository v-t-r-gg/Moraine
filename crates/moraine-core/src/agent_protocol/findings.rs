//! Human review findings: create, list, read, respond, and state change.
//!
//! Findings are descriptive review context attached to checkpoints (this slice).
//! They do not encode approval, rejection, acceptance, pass/fail, or correctness.
//! Mutations write the run sidecar under the existing per-record lock and append
//! named ledger events: `finding.created`, `finding.responded`, `finding.state_changed`.

use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::append_ops::is_redacted;
use super::ops::recover_incomplete_agent_op;
use super::project::{find_run_by_id, resolve_existing_project};
use super::projection::REDACTED_MARKER;
use super::types::{
    AgentRunState, FindingKind, FindingLedgerEvent, FindingRecord, FindingResponse, FindingState,
    FindingTarget, FindingTargetKind, IdempotencyRecord, FINDING_EVENT_CREATED,
    FINDING_EVENT_RESPONDED, FINDING_EVENT_STATE_CHANGED, MAX_FIELD_CHARS,
};
use crate::atomic::SidecarLock;
use crate::document::Document;
use crate::error::{Error, Result};
use crate::run_meta::{
    content_hash, load_or_migrate_locked, moraine_sidecar_path, write_run_meta_unlocked,
};

/// Ordinary API/desktop/agent projection marker for redacted checkpoint targets.
pub const REDACTED_CHECKPOINT_SUMMARY: &str = REDACTED_MARKER;

/// Maximum finding / response body length (plain text).
pub const MAX_FINDING_BODY_CHARS: usize = MAX_FIELD_CHARS;

/// Ordinary projection of a finding's checkpoint target (not the canonical sidecar record).
///
/// When the target checkpoint has been redacted, `checkpoint_summary` is
/// `[REDACTED]` and `target_redacted` is true. Canonical frozen content may
/// still exist in the run sidecar for integrity; ordinary readers must not
/// receive it through this DTO.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FindingTargetContext {
    pub kind: String,
    pub checkpoint_op_id: Uuid,
    pub snapshot_hash: String,
    pub checkpoint_summary: String,
    /// True when ordinary readers must not see the frozen checkpoint claim text.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub target_redacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FindingListItem {
    pub finding_id: Uuid,
    pub run_id: Uuid,
    pub kind: String,
    pub state: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub response_count: usize,
    pub target: FindingTargetContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FindingThreadItem {
    /// `finding` or `response`
    pub item_kind: String,
    pub id: Uuid,
    pub body: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding_kind: Option<String>,
}

/// Sanitized frozen-target projection for ordinary APIs.
///
/// When `redacted` is true, claim content fields are omitted; only identity /
/// timing metadata remain. The canonical sidecar may retain full content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FindingTargetSnapshotDto {
    pub op_id: Uuid,
    pub created_at: String,
    /// Snapshot hash recorded when the finding was created (not claim text).
    pub snapshot_hash: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub redacted: bool,
    /// Present only when the target checkpoint is not redacted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_questions: Vec<String>,
    /// Rationale choice/reason strings; empty when redacted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationales: Vec<String>,
    /// Evidence labels only (not full commands/results) when not redacted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FindingDetail {
    pub finding_id: Uuid,
    pub run_id: Uuid,
    pub kind: String,
    pub state: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub target: FindingTargetContext,
    /// Ordinary projection of the frozen checkpoint. Content withheld when redacted.
    pub target_snapshot: FindingTargetSnapshotDto,
    /// Chronological thread: human finding first, then agent responses by time.
    pub thread: Vec<FindingThreadItem>,
    pub responses: Vec<FindingResponse>,
    pub ledger_events: Vec<FindingLedgerEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FindingMutationResult {
    pub finding_id: Uuid,
    pub run_id: Uuid,
    pub state: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub idempotent_replay: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<Uuid>,
    pub finding: FindingDetail,
}

#[derive(Debug, Clone)]
pub struct CreateFindingRequest {
    pub kind: FindingKind,
    pub body: String,
    pub checkpoint_op_id: Uuid,
}

/// Create a typed finding on a checkpoint for a run (project + run id path).
pub fn create_finding(
    project: Option<&Path>,
    run_id: Uuid,
    req: CreateFindingRequest,
) -> Result<FindingMutationResult> {
    let project = resolve_existing_project(project)?;
    let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
    create_finding_at_path(&md_path, req)
}

/// Create a finding for the run record at `md_path` (desktop host path).
pub fn create_finding_at_path(
    md_path: &Path,
    req: CreateFindingRequest,
) -> Result<FindingMutationResult> {
    let body = validate_finding_body(&req.body)?;
    with_agent_locked(md_path, |run_id, agent, snapshot_hash| {
        let cp = agent
            .checkpoints
            .iter()
            .find(|c| c.op_id == req.checkpoint_op_id)
            .cloned()
            .ok_or_else(|| Error::InvalidFinding {
                message: format!(
                    "checkpoint {} is not present on this run",
                    req.checkpoint_op_id
                ),
            })?;

        let now = Utc::now();
        let finding_id = Uuid::new_v4();
        let finding = FindingRecord {
            id: finding_id,
            kind: req.kind,
            state: FindingState::Open,
            body,
            target: FindingTarget {
                kind: FindingTargetKind::Checkpoint,
                checkpoint_op_id: cp.op_id,
                snapshot_hash: snapshot_hash.to_string(),
                checkpoint: cp,
            },
            created_at: now,
            updated_at: now,
            responses: vec![],
        };

        agent.finding_events.push(FindingLedgerEvent {
            event_id: Uuid::new_v4(),
            event: FINDING_EVENT_CREATED.into(),
            finding_id,
            created_at: now,
            response_id: None,
            from_state: None,
            to_state: Some(FindingState::Open),
            kind: Some(req.kind),
            checkpoint_op_id: Some(req.checkpoint_op_id),
            snapshot_hash: Some(snapshot_hash.to_string()),
        });
        agent.findings.push(finding);

        Ok(FindingMutationResult {
            finding_id,
            run_id,
            state: FindingState::Open.as_str().into(),
            kind: req.kind.as_str().into(),
            idempotent_replay: false,
            response_id: None,
            finding: detail_from_agent(run_id, agent, finding_id)?,
        })
    })
}

/// List findings for a run. When `open_only` is true, only `open` findings are returned.
pub fn list_findings(
    project: Option<&Path>,
    run_id: Uuid,
    open_only: bool,
) -> Result<Vec<FindingListItem>> {
    let project = resolve_existing_project(project)?;
    let (md_path, meta) = find_run_by_id(&project.project_root, run_id)?;
    let _ = md_path;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other("run missing agent state"))?;
    Ok(list_from_agent(run_id, agent, open_only))
}

/// List findings by Markdown path (desktop).
pub fn list_findings_at_path(md_path: &Path, open_only: bool) -> Result<Vec<FindingListItem>> {
    let meta = load_run_meta_for_read(md_path)?;
    let run_id = meta.run.id;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other("run missing agent state"))?;
    Ok(list_from_agent(run_id, agent, open_only))
}

/// Read a complete finding thread plus original target snapshot.
pub fn get_finding(
    project: Option<&Path>,
    run_id: Uuid,
    finding_id: Uuid,
) -> Result<FindingDetail> {
    let project = resolve_existing_project(project)?;
    let (_md_path, meta) = find_run_by_id(&project.project_root, run_id)?;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other("run missing agent state"))?;
    detail_from_agent(run_id, agent, finding_id)
}

/// Read a finding by path (desktop).
pub fn get_finding_at_path(md_path: &Path, finding_id: Uuid) -> Result<FindingDetail> {
    let meta = load_run_meta_for_read(md_path)?;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other("run missing agent state"))?;
    detail_from_agent(meta.run.id, agent, finding_id)
}

/// Agent response to a finding. Requires a non-empty idempotency key.
/// Same key + same payload replays; same key + different payload conflicts.
pub fn respond_to_finding(
    project: Option<&Path>,
    run_id: Uuid,
    finding_id: Uuid,
    body: &str,
    idempotency_key: &str,
) -> Result<FindingMutationResult> {
    let project = resolve_existing_project(project)?;
    let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
    respond_to_finding_at_path(&md_path, finding_id, body, idempotency_key)
}

pub fn respond_to_finding_at_path(
    md_path: &Path,
    finding_id: Uuid,
    body: &str,
    idempotency_key: &str,
) -> Result<FindingMutationResult> {
    if idempotency_key.trim().is_empty() {
        return Err(Error::InvalidFinding {
            message: "idempotency key is required".into(),
        });
    }
    let body = validate_finding_body(body)?;
    let payload_hash = hash_payload(&json!({
        "kind": "finding_respond",
        "findingId": finding_id.to_string(),
        "body": body,
    }));
    let idem_key = idempotency_key.to_string();

    with_agent_locked(md_path, |run_id, agent, _hash| {
        // Lifetime idempotency (same discipline as agent ops).
        if let Some(prev) = agent.find_idempotency(&idem_key).cloned() {
            if prev.payload_hash != payload_hash {
                return Err(Error::IdempotencyConflict {
                    key: idem_key,
                    message: "key was used for a different finding response payload".into(),
                });
            }
            // Replay: return existing response result without appending again.
            let detail = detail_from_agent(run_id, agent, finding_id)?;
            let response_id = detail
                .responses
                .iter()
                .find(|r| r.idempotency_key == idem_key)
                .map(|r| r.id)
                .or(Some(prev.op_id));
            let f = agent
                .findings
                .iter()
                .find(|f| f.id == finding_id)
                .ok_or(Error::FindingNotFound { id: finding_id })?;
            return Ok(FindingMutationResult {
                finding_id,
                run_id,
                state: f.state.as_str().into(),
                kind: f.kind.as_str().into(),
                idempotent_replay: true,
                response_id,
                finding: detail,
            });
        }

        if !agent.has_idempotency_capacity_for(&idem_key) {
            return Err(Error::IdempotencyIndexFull {
                max: super::types::MAX_IDEMPOTENCY_INDEX,
            });
        }

        // Ensure finding exists before mutating.
        if !agent.findings.iter().any(|f| f.id == finding_id) {
            return Err(Error::FindingNotFound { id: finding_id });
        }

        let now = Utc::now();
        let response_id = Uuid::new_v4();
        let (kind, state) = {
            let finding = agent
                .findings
                .iter_mut()
                .find(|f| f.id == finding_id)
                .ok_or(Error::FindingNotFound { id: finding_id })?;
            finding.responses.push(FindingResponse {
                id: response_id,
                finding_id,
                body,
                created_at: now,
                idempotency_key: idem_key.clone(),
                author_kind: "agent".into(),
            });
            finding.updated_at = now;
            (finding.kind, finding.state)
        };

        agent.finding_events.push(FindingLedgerEvent {
            event_id: Uuid::new_v4(),
            event: FINDING_EVENT_RESPONDED.into(),
            finding_id,
            created_at: now,
            response_id: Some(response_id),
            from_state: None,
            to_state: None,
            kind: Some(kind),
            checkpoint_op_id: None,
            snapshot_hash: None,
        });

        agent.record_idempotency(
            idem_key,
            IdempotencyRecord {
                payload_hash,
                op_id: response_id,
                kind: "finding_respond".into(),
                content_hash: content_hash_placeholder(agent),
                record_revision: agent.record_revision,
                created_at: now,
            },
        )?;

        Ok(FindingMutationResult {
            finding_id,
            run_id,
            state: state.as_str().into(),
            kind: kind.as_str().into(),
            idempotent_replay: false,
            response_id: Some(response_id),
            finding: detail_from_agent(run_id, agent, finding_id)?,
        })
    })
}

/// Explicitly change finding state (`open`, `addressed`, `archived`).
/// `addressed` means only that attention was marked—not approval.
pub fn change_finding_state(
    project: Option<&Path>,
    run_id: Uuid,
    finding_id: Uuid,
    new_state: FindingState,
) -> Result<FindingMutationResult> {
    let project = resolve_existing_project(project)?;
    let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
    change_finding_state_at_path(&md_path, finding_id, new_state)
}

pub fn change_finding_state_at_path(
    md_path: &Path,
    finding_id: Uuid,
    new_state: FindingState,
) -> Result<FindingMutationResult> {
    with_agent_locked(md_path, |run_id, agent, _hash| {
        let (from, kind) = {
            let finding = agent
                .findings
                .iter()
                .find(|f| f.id == finding_id)
                .ok_or(Error::FindingNotFound { id: finding_id })?;
            (finding.state, finding.kind)
        };

        if from == new_state {
            return Ok(FindingMutationResult {
                finding_id,
                run_id,
                state: new_state.as_str().into(),
                kind: kind.as_str().into(),
                idempotent_replay: true,
                response_id: None,
                finding: detail_from_agent(run_id, agent, finding_id)?,
            });
        }

        let now = Utc::now();
        {
            let finding = agent
                .findings
                .iter_mut()
                .find(|f| f.id == finding_id)
                .ok_or(Error::FindingNotFound { id: finding_id })?;
            finding.state = new_state;
            finding.updated_at = now;
        }

        agent.finding_events.push(FindingLedgerEvent {
            event_id: Uuid::new_v4(),
            event: FINDING_EVENT_STATE_CHANGED.into(),
            finding_id,
            created_at: now,
            response_id: None,
            from_state: Some(from),
            to_state: Some(new_state),
            kind: Some(kind),
            checkpoint_op_id: None,
            snapshot_hash: None,
        });

        Ok(FindingMutationResult {
            finding_id,
            run_id,
            state: new_state.as_str().into(),
            kind: kind.as_str().into(),
            idempotent_replay: false,
            response_id: None,
            finding: detail_from_agent(run_id, agent, finding_id)?,
        })
    })
}

/// Compact checkpoint list for desktop checkpoint-detail UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointSummaryDto {
    pub op_id: Uuid,
    pub summary: String,
    pub created_at: String,
    pub open_finding_count: usize,
    pub finding_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunCheckpointsDetail {
    pub run_id: Uuid,
    pub content_hash: String,
    pub checkpoints: Vec<CheckpointSummaryDto>,
    pub findings: Vec<FindingListItem>,
}

/// Load checkpoints + findings for a run record path (desktop).
pub fn load_run_checkpoints_detail(md_path: &Path) -> Result<RunCheckpointsDetail> {
    let meta = load_run_meta_for_read(md_path)?;
    let markdown = Document::read_file(md_path)?;
    let hash = content_hash(&markdown);
    let run_id = meta.run.id;
    let agent = meta.agent.as_ref();
    let findings = agent
        .map(|a| list_from_agent(run_id, a, false))
        .unwrap_or_default();
    let checkpoints = agent
        .map(|a| {
            a.checkpoints
                .iter()
                .map(|cp| {
                    let related: Vec<_> = a
                        .findings
                        .iter()
                        .filter(|f| f.target.checkpoint_op_id == cp.op_id)
                        .collect();
                    // Ordinary UI must not re-expose redacted claim text; original
                    // summary remains available when not redacted (amend chains keep
                    // Original claim vs Current statement in the protocol ledger UI).
                    let summary = if crate::agent_protocol::append_ops::is_redacted(a, cp.op_id) {
                        "[REDACTED]".into()
                    } else {
                        cp.summary.clone()
                    };
                    CheckpointSummaryDto {
                        op_id: cp.op_id,
                        summary,
                        created_at: cp.created_at.to_rfc3339(),
                        open_finding_count: related
                            .iter()
                            .filter(|f| f.state == FindingState::Open)
                            .count(),
                        finding_count: related.len(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(RunCheckpointsDetail {
        run_id,
        content_hash: hash,
        checkpoints,
        findings,
    })
}

fn list_from_agent(run_id: Uuid, agent: &AgentRunState, open_only: bool) -> Vec<FindingListItem> {
    agent
        .findings
        .iter()
        .filter(|f| !open_only || f.state == FindingState::Open)
        .map(|f| FindingListItem {
            finding_id: f.id,
            run_id,
            kind: f.kind.as_str().into(),
            state: f.state.as_str().into(),
            body: f.body.clone(),
            created_at: f.created_at.to_rfc3339(),
            updated_at: f.updated_at.to_rfc3339(),
            response_count: f.responses.len(),
            target: project_target_context(agent, &f.target),
        })
        .collect()
}

fn detail_from_agent(
    run_id: Uuid,
    agent: &AgentRunState,
    finding_id: Uuid,
) -> Result<FindingDetail> {
    let f = agent
        .findings
        .iter()
        .find(|x| x.id == finding_id)
        .ok_or(Error::FindingNotFound { id: finding_id })?;

    let mut thread = Vec::with_capacity(1 + f.responses.len());
    thread.push(FindingThreadItem {
        item_kind: "finding".into(),
        id: f.id,
        body: f.body.clone(),
        created_at: f.created_at.to_rfc3339(),
        author_kind: Some("human".into()),
        finding_kind: Some(f.kind.as_str().into()),
    });
    let mut responses = f.responses.clone();
    responses.sort_by_key(|r| r.created_at);
    for r in &responses {
        thread.push(FindingThreadItem {
            item_kind: "response".into(),
            id: r.id,
            body: r.body.clone(),
            created_at: r.created_at.to_rfc3339(),
            author_kind: Some(r.author_kind.clone()),
            finding_kind: None,
        });
    }

    let ledger_events: Vec<_> = agent
        .finding_events
        .iter()
        .filter(|e| e.finding_id == finding_id)
        .cloned()
        .collect();

    Ok(FindingDetail {
        finding_id: f.id,
        run_id,
        kind: f.kind.as_str().into(),
        state: f.state.as_str().into(),
        body: f.body.clone(),
        created_at: f.created_at.to_rfc3339(),
        updated_at: f.updated_at.to_rfc3339(),
        target: project_target_context(agent, &f.target),
        target_snapshot: project_target_snapshot(agent, &f.target),
        thread,
        responses,
        ledger_events,
    })
}

/// Single redaction-aware projection for ordinary finding target context.
///
/// Used by list/get/respond/desktop/MCP. Does not mutate the sidecar.
pub fn project_target_context(agent: &AgentRunState, t: &FindingTarget) -> FindingTargetContext {
    let redacted = is_redacted(agent, t.checkpoint_op_id);
    FindingTargetContext {
        kind: t.kind.as_str().into(),
        checkpoint_op_id: t.checkpoint_op_id,
        snapshot_hash: t.snapshot_hash.clone(),
        checkpoint_summary: if redacted {
            REDACTED_CHECKPOINT_SUMMARY.into()
        } else {
            t.checkpoint.summary.clone()
        },
        target_redacted: redacted,
    }
}

/// Single redaction-aware projection for frozen checkpoint snapshots.
///
/// Redacted targets keep only identity/timestamp/hash — no summary, actions,
/// rationales, evidence, risks, or questions.
pub fn project_target_snapshot(
    agent: &AgentRunState,
    t: &FindingTarget,
) -> FindingTargetSnapshotDto {
    let redacted = is_redacted(agent, t.checkpoint_op_id);
    let cp = &t.checkpoint;
    if redacted {
        return FindingTargetSnapshotDto {
            op_id: cp.op_id,
            created_at: cp.created_at.to_rfc3339(),
            snapshot_hash: t.snapshot_hash.clone(),
            redacted: true,
            summary: None,
            actions: vec![],
            risks: vec![],
            open_questions: vec![],
            rationales: vec![],
            evidence_labels: vec![],
        };
    }
    FindingTargetSnapshotDto {
        op_id: cp.op_id,
        created_at: cp.created_at.to_rfc3339(),
        snapshot_hash: t.snapshot_hash.clone(),
        redacted: false,
        summary: Some(cp.summary.clone()),
        actions: cp.actions.clone(),
        risks: cp.risks.clone(),
        open_questions: cp.open_questions.clone(),
        rationales: cp
            .rationales
            .iter()
            .map(|r| format!("{}: {}", r.choice, r.reason))
            .collect(),
        evidence_labels: cp.evidence.iter().map(|e| e.label.clone()).collect(),
    }
}

fn validate_finding_body(body: &str) -> Result<String> {
    let t = body.trim();
    if t.is_empty() {
        return Err(Error::InvalidFinding {
            message: "finding body is required".into(),
        });
    }
    if t.chars().count() > MAX_FINDING_BODY_CHARS {
        return Err(Error::InvalidFinding {
            message: format!("finding body exceeds {MAX_FINDING_BODY_CHARS} characters"),
        });
    }
    for ch in t.chars() {
        if ch == '\0' || (ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t') {
            return Err(Error::InvalidFinding {
                message: "finding body must not contain control characters".into(),
            });
        }
    }
    Ok(t.to_string())
}

fn hash_payload(v: &serde_json::Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

/// Idempotency records require a content_hash; findings do not rewrite Markdown.
fn content_hash_placeholder(agent: &AgentRunState) -> String {
    format!("finding-rev-{}", agent.record_revision)
}

fn load_run_meta_for_read(md_path: &Path) -> Result<crate::run_meta::RunMeta> {
    crate::run_meta::load_run_meta_readonly(md_path)?.ok_or_else(|| Error::NotFound(md_path.into()))
}

fn with_agent_locked<F>(md_path: &Path, f: F) -> Result<FindingMutationResult>
where
    F: FnOnce(Uuid, &mut AgentRunState, &str) -> Result<FindingMutationResult>,
{
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = load_or_migrate_locked(md_path)?;
    let markdown = Document::read_file(md_path)?;
    let snapshot_hash = content_hash(&markdown);

    // Recover incomplete agent ops first so we never mutate only committed state
    // while pending_agent would later overwrite findings on promote.
    recover_incomplete_agent_op(md_path, &mut meta, &snapshot_hash)?;
    meta = load_or_migrate_locked(md_path)?;
    let markdown = Document::read_file(md_path)?;
    let snapshot_hash = content_hash(&markdown);

    let run_id = meta.run.id;
    let agent = meta
        .agent
        .as_mut()
        .ok_or_else(|| Error::other("run missing agent state"))?;

    // Belt-and-suspenders: if incomplete_op somehow remains, also apply the same
    // mutation into pending_agent so a later promote cannot wipe findings.
    // After normal recovery incomplete_op is None; this only helps if recovery
    // was a no-op for an unexpected residual (should not happen).
    let result = f(run_id, agent, &snapshot_hash)?;

    // If an incomplete_op still exists (recovery discarded only on base/expected),
    // mirror findings + finding_events + related idempotency into pending so
    // promote keeps human review context.
    if let Some(agent) = meta.agent.as_mut() {
        if let Some(inc) = agent.incomplete_op.as_mut() {
            let pending = inc.pending_agent.as_mut();
            pending.findings = agent.findings.clone();
            pending.finding_events = agent.finding_events.clone();
            // Copy finding_respond idempotency keys so replay stays consistent.
            for (k, v) in &agent.idempotency {
                if v.kind == "finding_respond" {
                    pending.idempotency.insert(k.clone(), v.clone());
                }
            }
        }
    }

    meta.touch();
    write_run_meta_unlocked(md_path, &meta)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_protocol::markdown::render_run_markdown_with_id;
    use crate::agent_protocol::ops::{
        run_checkpoint, run_start, test_begin_incomplete_without_markdown, CheckpointInput,
        RunStartRequest,
    };
    use crate::agent_protocol::project::init_project;
    use crate::agent_protocol::types::{IncompleteOp, IncompletePhase};
    use crate::atomic::write_atomic;
    use crate::run_meta::load_run_meta_readonly;
    use tempfile::tempdir;

    fn finding_run_path(project: Option<&Path>, run_id: Uuid) -> Result<PathBuf> {
        let project = resolve_existing_project(project)?;
        let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
        Ok(md_path)
    }

    fn start_with_checkpoint(dir: &Path) -> (Uuid, PathBuf, Uuid, String) {
        let project = init_project(Some(dir)).unwrap();
        let start = run_start(RunStartRequest {
            objective: "finding test run".into(),
            idempotency_key: "find-start-1".into(),
            project: Some(project.project_root.clone()),
            session_id: None,
        })
        .unwrap();
        let cp = run_checkpoint(
            Some(&project.project_root),
            start.run_id,
            &start.content_hash,
            "find-cp-1",
            CheckpointInput {
                summary: "Implemented widget".into(),
                actions: vec!["wrote code".into()],
                rationales: vec![],
                evidence: vec![],
                risks: vec!["maybe flaky".into()],
                open_questions: vec![],
            },
        )
        .unwrap();
        let op_id = cp.op_id.unwrap();
        (start.run_id, project.project_root, op_id, cp.content_hash)
    }

    #[test]
    fn create_list_get_respond_state_persist_and_ledger() {
        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, _hash) = start_with_checkpoint(dir.path());

        let created = create_finding(
            Some(&root),
            run_id,
            CreateFindingRequest {
                kind: FindingKind::MissingEvidence,
                body: "What command proved the widget works?".into(),
                checkpoint_op_id: cp_id,
            },
        )
        .unwrap();
        assert_eq!(created.state, "open");
        assert_eq!(created.kind, "missing_evidence");
        assert_eq!(created.finding.target.checkpoint_op_id, cp_id);
        assert!(!created.finding.target.snapshot_hash.is_empty());
        assert_eq!(created.finding.target_snapshot.op_id, cp_id);
        assert_eq!(
            created.finding.target_snapshot.summary.as_deref(),
            Some("Implemented widget")
        );
        assert!(!created.finding.target.target_redacted);
        assert!(!created.finding.target_snapshot.redacted);

        let open = list_findings(Some(&root), run_id, true).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].finding_id, created.finding_id);
        assert_eq!(open[0].target.checkpoint_summary, "Implemented widget");

        let fid = created.finding_id;
        let r1 = respond_to_finding(
            Some(&root),
            run_id,
            fid,
            "Ran cargo test -p widget; exit 0.",
            "resp-key-1",
        )
        .unwrap();
        assert!(!r1.idempotent_replay);
        assert!(r1.response_id.is_some());

        // Same key + same body → replay
        let r_replay = respond_to_finding(
            Some(&root),
            run_id,
            fid,
            "Ran cargo test -p widget; exit 0.",
            "resp-key-1",
        )
        .unwrap();
        assert!(r_replay.idempotent_replay);
        assert_eq!(r_replay.response_id, r1.response_id);

        // Same key + different body → conflict
        let conflict = respond_to_finding(Some(&root), run_id, fid, "Different body", "resp-key-1");
        assert!(matches!(conflict, Err(Error::IdempotencyConflict { .. })));

        let addressed =
            change_finding_state(Some(&root), run_id, fid, FindingState::Addressed).unwrap();
        assert_eq!(addressed.state, "addressed");
        assert!(list_findings(Some(&root), run_id, true).unwrap().is_empty());
        assert_eq!(list_findings(Some(&root), run_id, false).unwrap().len(), 1);

        // Durability across reload (re-open sidecar)
        let detail = get_finding(Some(&root), run_id, fid).unwrap();
        assert_eq!(detail.thread.len(), 2);
        assert_eq!(detail.thread[0].item_kind, "finding");
        assert_eq!(detail.thread[0].author_kind.as_deref(), Some("human"));
        assert_eq!(detail.thread[1].item_kind, "response");
        assert_eq!(detail.thread[1].author_kind.as_deref(), Some("agent"));
        assert_eq!(detail.state, "addressed");
        assert_eq!(
            detail.target_snapshot.summary.as_deref(),
            Some("Implemented widget")
        );

        let events: Vec<_> = detail
            .ledger_events
            .iter()
            .map(|e| e.event.as_str())
            .collect();
        assert_eq!(
            events,
            vec![
                FINDING_EVENT_CREATED,
                FINDING_EVENT_RESPONDED,
                FINDING_EVENT_STATE_CHANGED
            ]
        );

        // Reload from disk independently
        let md = finding_run_path(Some(&root), run_id).unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let agent = meta.agent.unwrap();
        assert_eq!(agent.findings.len(), 1);
        assert_eq!(agent.findings[0].responses.len(), 1);
        assert_eq!(agent.finding_events.len(), 3);

        // No approval/rejection/pass-fail on finding types (structural)
        let serialized = serde_json::to_string(&agent.findings[0]).unwrap();
        for banned in [
            "approved",
            "rejected",
            "acceptance",
            "pass_fail",
            "pass/fail",
            "correctness",
        ] {
            assert!(
                !serialized.contains(banned),
                "finding JSON must not contain {banned}: {serialized}"
            );
        }
    }

    #[test]
    fn rejects_unknown_checkpoint_and_missing_idempotency() {
        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, _hash) = start_with_checkpoint(dir.path());
        let missing = Uuid::new_v4();
        let err = create_finding(
            Some(&root),
            run_id,
            CreateFindingRequest {
                kind: FindingKind::Clarification,
                body: "Why?".into(),
                checkpoint_op_id: missing,
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::InvalidFinding { .. }), "{err:?}");

        let created = create_finding(
            Some(&root),
            run_id,
            CreateFindingRequest {
                kind: FindingKind::Other,
                body: "note".into(),
                checkpoint_op_id: cp_id,
            },
        )
        .unwrap();

        let err = respond_to_finding(Some(&root), run_id, created.finding_id, "hi", "  ");
        assert!(matches!(err, Err(Error::InvalidFinding { .. })));
    }

    #[test]
    fn kind_and_state_parsers_cover_required_set() {
        for k in [
            "clarification",
            "inconsistency",
            "missing_evidence",
            "risk_concern",
            "factual_correction",
            "other",
        ] {
            assert_eq!(FindingKind::parse(k).unwrap().as_str(), k);
        }
        for s in ["open", "addressed", "archived"] {
            assert_eq!(FindingState::parse(s).unwrap().as_str(), s);
        }
        assert!(FindingKind::parse("approval").is_none());
        assert!(FindingState::parse("approved").is_none());
    }

    /// Regression: create finding while incomplete_op is pending with Markdown already
    /// at expected hash; recovery promotes pending then records the finding so a later
    /// agent op cannot wipe it.
    #[test]
    fn finding_survives_incomplete_op_promotion_recovery() {
        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, base_hash) = start_with_checkpoint(dir.path());
        let md = finding_run_path(Some(&root), run_id).unwrap();

        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let committed = meta.agent.as_ref().unwrap().clone();
        let mut pending = committed.clone();
        // Pending agent mutation (checkpoint already applied in MD later).
        pending.bump_revision().unwrap();
        pending.ready_summary = Some("ghost ready".into());
        let pending_md = render_run_markdown_with_id(run_id, &pending, "");
        let expected = content_hash(&pending_md);

        test_begin_incomplete_without_markdown(
            &md,
            pending,
            IncompleteOp {
                op_id: Uuid::new_v4(),
                idempotency_key: "ghost-ready".into(),
                kind: "ready".into(),
                payload_hash: "ghost".into(),
                base_content_hash: base_hash.clone(),
                expected_content_hash: expected.clone(),
                phase: IncompletePhase::Begun,
                created_at: Utc::now(),
                // placeholder; helper overwrites with pending
                pending_agent: Box::new(committed.clone()),
            },
        )
        .unwrap();
        // Crash after Markdown write, before promote.
        write_atomic(&md, pending_md.as_bytes()).unwrap();

        let created = create_finding(
            Some(&root),
            run_id,
            CreateFindingRequest {
                kind: FindingKind::RiskConcern,
                body: "What is residual risk after recovery?".into(),
                checkpoint_op_id: cp_id,
            },
        )
        .expect("create_finding during incomplete must recover then persist");
        assert_eq!(created.finding.target.checkpoint_op_id, cp_id);

        // After create: incomplete cleared, finding present, pending fields promoted.
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let agent = meta.agent.as_ref().unwrap();
        assert!(
            agent.incomplete_op.is_none(),
            "incomplete_op must be recovered"
        );
        assert_eq!(agent.findings.len(), 1);
        assert_eq!(agent.findings[0].id, created.finding_id);
        assert_eq!(agent.ready_summary.as_deref(), Some("ghost ready"));
        assert_eq!(agent.finding_events.len(), 1);
        assert_eq!(agent.finding_events[0].event, FINDING_EVENT_CREATED);

        // Later agent mutation must not wipe the finding.
        let hash = content_hash(&Document::read_file(&md).unwrap());
        let _ = run_checkpoint(
            Some(&root),
            run_id,
            &hash,
            "after-recover-cp",
            CheckpointInput {
                summary: "post recovery work".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
            },
        )
        .unwrap();
        let detail = get_finding(Some(&root), run_id, created.finding_id).unwrap();
        assert_eq!(detail.body, "What is residual risk after recovery?");
        assert_eq!(detail.ledger_events.len(), 1);
    }

    /// Create while incomplete at base hash (MD never applied): discard pending, keep finding.
    #[test]
    fn finding_survives_incomplete_op_discard_then_agent_op() {
        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, base_hash) = start_with_checkpoint(dir.path());
        let md = finding_run_path(Some(&root), run_id).unwrap();

        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let committed = meta.agent.as_ref().unwrap().clone();
        let mut pending = committed.clone();
        pending
            .checkpoints
            .push(crate::agent_protocol::types::CheckpointRecord {
                op_id: Uuid::new_v4(),
                idempotency_key: "ghost".into(),
                created_at: Utc::now(),
                summary: "should not appear".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
                git: None,
            });
        pending.bump_revision().unwrap();
        let pending_md = render_run_markdown_with_id(run_id, &pending, "");
        let expected = content_hash(&pending_md);

        test_begin_incomplete_without_markdown(
            &md,
            pending,
            IncompleteOp {
                op_id: Uuid::new_v4(),
                idempotency_key: "ghost".into(),
                kind: "checkpoint".into(),
                payload_hash: "x".into(),
                base_content_hash: base_hash.clone(),
                expected_content_hash: expected,
                phase: IncompletePhase::Begun,
                created_at: Utc::now(),
                pending_agent: Box::new(committed.clone()),
            },
        )
        .unwrap();
        // MD still at base — incomplete discard path.

        let created = create_finding(
            Some(&root),
            run_id,
            CreateFindingRequest {
                kind: FindingKind::Clarification,
                body: "Still durable after discard recovery?".into(),
                checkpoint_op_id: cp_id,
            },
        )
        .unwrap();

        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let agent = meta.agent.as_ref().unwrap();
        assert!(agent.incomplete_op.is_none());
        assert_eq!(agent.findings.len(), 1);
        assert!(!agent
            .checkpoints
            .iter()
            .any(|c| c.summary == "should not appear"));

        // Subsequent agent op still keeps the finding.
        let hash = content_hash(&Document::read_file(&md).unwrap());
        run_checkpoint(
            Some(&root),
            run_id,
            &hash,
            "after-discard",
            CheckpointInput {
                summary: "real after discard".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
            },
        )
        .unwrap();
        let detail = get_finding(Some(&root), run_id, created.finding_id).unwrap();
        assert_eq!(detail.body, "Still durable after discard recovery?");
    }

    /// Redacted checkpoint targets must not leak claim content through ordinary
    /// finding projections (list/get/respond paths share detail_from_agent).
    #[test]
    fn redacted_checkpoint_withheld_from_finding_projections() {
        use crate::agent_protocol::append_ops::{entry_redact, RedactRequest};
        use crate::agent_protocol::types::ActorCategory;

        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, _hash) = start_with_checkpoint(dir.path());
        let secret = "Implemented widget";

        let created = create_finding(
            Some(&root),
            run_id,
            CreateFindingRequest {
                kind: FindingKind::MissingEvidence,
                body: "What proved the widget works?".into(),
                checkpoint_op_id: cp_id,
            },
        )
        .unwrap();
        assert_eq!(created.finding.target.checkpoint_summary.as_str(), secret);
        assert_eq!(
            created.finding.target_snapshot.summary.as_deref(),
            Some(secret)
        );
        assert!(!created.finding.target_snapshot.actions.is_empty());

        entry_redact(
            Some(&root),
            run_id,
            RedactRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "sensitive claim".into(),
                actor_category: ActorCategory::Human,
            },
        )
        .unwrap();

        // list_findings
        let listed = list_findings(Some(&root), run_id, false).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].target.target_redacted);
        assert_eq!(
            listed[0].target.checkpoint_summary,
            REDACTED_CHECKPOINT_SUMMARY
        );
        assert!(!listed[0].target.checkpoint_summary.contains(secret));

        // get_finding
        let detail = get_finding(Some(&root), run_id, created.finding_id).unwrap();
        assert!(detail.target.target_redacted);
        assert_eq!(
            detail.target.checkpoint_summary,
            REDACTED_CHECKPOINT_SUMMARY
        );
        assert_eq!(detail.target.checkpoint_op_id, cp_id);
        assert!(!detail.target.snapshot_hash.is_empty());
        assert!(detail.target_snapshot.redacted);
        assert!(detail.target_snapshot.summary.is_none());
        assert!(detail.target_snapshot.actions.is_empty());
        assert!(detail.target_snapshot.risks.is_empty());
        assert!(detail.target_snapshot.open_questions.is_empty());
        assert!(detail.target_snapshot.rationales.is_empty());
        assert!(detail.target_snapshot.evidence_labels.is_empty());

        // respond_to_finding returns projected FindingDetail
        let responded = respond_to_finding(
            Some(&root),
            run_id,
            created.finding_id,
            "cargo test -p widget exited 0",
            "resp-after-redact",
        )
        .unwrap();
        assert!(responded.finding.target.target_redacted);
        assert_eq!(
            responded.finding.target.checkpoint_summary,
            REDACTED_CHECKPOINT_SUMMARY
        );
        assert!(responded.finding.target_snapshot.redacted);
        assert!(responded.finding.target_snapshot.summary.is_none());

        // JSON must not re-expose secret claim content
        let json = serde_json::to_string(&detail).unwrap();
        assert!(
            !json.contains(secret),
            "serialized get_finding leaked redacted claim: {json}"
        );
        let list_json = serde_json::to_string(&listed).unwrap();
        assert!(
            !list_json.contains(secret),
            "serialized list_findings leaked redacted claim: {list_json}"
        );

        // Sidecar still retains canonical frozen snapshot for integrity.
        let md = finding_run_path(Some(&root), run_id).unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let agent = meta.agent.as_ref().unwrap();
        let stored = agent
            .findings
            .iter()
            .find(|f| f.id == created.finding_id)
            .unwrap();
        assert_eq!(stored.target.checkpoint.summary, secret);
        assert!(is_redacted(agent, cp_id));
    }
}
