//! Append-only ledger operations (M4.6).
//!
//! Protocol agent claims (checkpoints, rationale, evidence, mechanical events) are
//! never rewritten in place. Humans add observations; agents or humans may amend,
//! supersede, or redact with explicit recoverable history.

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::markdown::{extract_human_notes, render_run_markdown_with_id};
use super::ops::recover_incomplete_agent_op;
use super::project::{find_run_by_id, resolve_existing_project};
use super::types::{
    ActorCategory, AgentRunState, AppendOnlyOpRecord, LedgerRelationship, MAX_FIELD_CHARS,
    OP_ENTRY_REDACT, OP_ENTRY_SUPERSEDE, OP_HUMAN_OBSERVATION_ADD, OP_RUN_AMEND,
};
use crate::atomic::{write_atomic, SidecarLock};
use crate::document::Document;
use crate::error::{Error, Result};
use crate::run_meta::{
    content_hash, load_or_migrate_locked, moraine_sidecar_path, write_run_meta_unlocked,
};

pub const MAX_OP_BODY_CHARS: usize = MAX_FIELD_CHARS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppendOpResult {
    pub run_id: Uuid,
    pub op_id: Uuid,
    pub op_kind: String,
    pub relationship: String,
    pub op: AppendOnlyOpRecord,
}

#[derive(Debug, Clone)]
pub struct HumanObservationRequest {
    pub body: String,
    pub reason: String,
    pub target_id: Option<Uuid>,
    pub target_kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AmendRequest {
    pub target_id: Uuid,
    /// Currently only `checkpoint` (immutable claim amended by append).
    pub target_kind: String,
    pub reason: String,
    pub new_content: String,
    pub actor_category: ActorCategory,
}

#[derive(Debug, Clone)]
pub struct SupersedeRequest {
    pub target_id: Uuid,
    pub target_kind: String,
    pub reason: String,
    pub new_content: String,
    pub actor_category: ActorCategory,
}

#[derive(Debug, Clone)]
pub struct RedactRequest {
    pub target_id: Uuid,
    pub target_kind: String,
    pub reason: String,
    pub actor_category: ActorCategory,
}

/// List all append-only ops for a run (project path).
pub fn list_append_ops(project: Option<&Path>, run_id: Uuid) -> Result<Vec<AppendOnlyOpRecord>> {
    let project = resolve_existing_project(project)?;
    let (_md, meta) = find_run_by_id(&project.project_root, run_id)?;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other("run missing agent state"))?;
    Ok(agent.append_only_ops.clone())
}

pub fn list_append_ops_at_path(md_path: &Path) -> Result<Vec<AppendOnlyOpRecord>> {
    let meta = crate::run_meta::load_run_meta_readonly(md_path)?
        .ok_or_else(|| Error::NotFound(md_path.into()))?;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other("run missing agent state"))?;
    Ok(agent.append_only_ops.clone())
}

pub fn human_observation_add(
    project: Option<&Path>,
    run_id: Uuid,
    req: HumanObservationRequest,
) -> Result<AppendOpResult> {
    let project = resolve_existing_project(project)?;
    let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
    human_observation_add_at_path(&md_path, req)
}

pub fn human_observation_add_at_path(
    md_path: &Path,
    req: HumanObservationRequest,
) -> Result<AppendOpResult> {
    let body = validate_body("observation body", &req.body)?;
    let reason = validate_body("reason", &req.reason)?;
    append_op(md_path, |run_id, agent, snapshot_hash| {
        if let Some(tid) = req.target_id {
            ensure_target_exists(
                agent,
                tid,
                req.target_kind.as_deref().unwrap_or("checkpoint"),
            )?;
        }
        let op = AppendOnlyOpRecord {
            op_id: Uuid::new_v4(),
            op_kind: OP_HUMAN_OBSERVATION_ADD.into(),
            actor_category: ActorCategory::Human,
            created_at: Utc::now(),
            reason,
            target_id: req.target_id,
            target_kind: req.target_kind,
            previous_snapshot_hash: snapshot_hash.to_string(),
            previous_content: None,
            new_content: Some(body),
            relationship: LedgerRelationship::Observation,
        };
        agent.append_only_ops.push(op.clone());
        Ok(AppendOpResult {
            run_id,
            op_id: op.op_id,
            op_kind: op.op_kind.clone(),
            relationship: op.relationship.as_str().into(),
            op,
        })
    })
}

pub fn run_amend(
    project: Option<&Path>,
    run_id: Uuid,
    req: AmendRequest,
) -> Result<AppendOpResult> {
    let project = resolve_existing_project(project)?;
    let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
    run_amend_at_path(&md_path, req)
}

pub fn run_amend_at_path(md_path: &Path, req: AmendRequest) -> Result<AppendOpResult> {
    let reason = validate_body("reason", &req.reason)?;
    let new_content = validate_body("new content", &req.new_content)?;
    if req.target_kind.trim() != "checkpoint" {
        return Err(Error::InvalidFinding {
            message: format!(
                "run_amend currently supports target_kind=checkpoint only, got {:?}",
                req.target_kind
            ),
        });
    }
    append_op(md_path, |run_id, agent, snapshot_hash| {
        // Freeze the claim immediately prior to this op (original or latest amend/supersede),
        // never only the immutable original checkpoint summary.
        let prev = resolve_target_content(agent, req.target_id, "checkpoint")?;
        // Never mutate the checkpoint record itself.
        let op = AppendOnlyOpRecord {
            op_id: Uuid::new_v4(),
            op_kind: OP_RUN_AMEND.into(),
            actor_category: req.actor_category,
            created_at: Utc::now(),
            reason,
            target_id: Some(req.target_id),
            target_kind: Some("checkpoint".into()),
            previous_snapshot_hash: snapshot_hash.to_string(),
            previous_content: Some(prev),
            new_content: Some(new_content),
            relationship: LedgerRelationship::Amended,
        };
        agent.append_only_ops.push(op.clone());
        Ok(AppendOpResult {
            run_id,
            op_id: op.op_id,
            op_kind: op.op_kind.clone(),
            relationship: op.relationship.as_str().into(),
            op,
        })
    })
}

pub fn entry_supersede(
    project: Option<&Path>,
    run_id: Uuid,
    req: SupersedeRequest,
) -> Result<AppendOpResult> {
    let project = resolve_existing_project(project)?;
    let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
    entry_supersede_at_path(&md_path, req)
}

pub fn entry_supersede_at_path(md_path: &Path, req: SupersedeRequest) -> Result<AppendOpResult> {
    let reason = validate_body("reason", &req.reason)?;
    let new_content = validate_body("new content", &req.new_content)?;
    append_op(md_path, |run_id, agent, snapshot_hash| {
        let prev = resolve_target_content(agent, req.target_id, &req.target_kind)?;
        let op = AppendOnlyOpRecord {
            op_id: Uuid::new_v4(),
            op_kind: OP_ENTRY_SUPERSEDE.into(),
            actor_category: req.actor_category,
            created_at: Utc::now(),
            reason,
            target_id: Some(req.target_id),
            target_kind: Some(req.target_kind.trim().to_string()),
            previous_snapshot_hash: snapshot_hash.to_string(),
            previous_content: Some(prev),
            new_content: Some(new_content),
            relationship: LedgerRelationship::Superseded,
        };
        agent.append_only_ops.push(op.clone());
        Ok(AppendOpResult {
            run_id,
            op_id: op.op_id,
            op_kind: op.op_kind.clone(),
            relationship: op.relationship.as_str().into(),
            op,
        })
    })
}

pub fn entry_redact(
    project: Option<&Path>,
    run_id: Uuid,
    req: RedactRequest,
) -> Result<AppendOpResult> {
    let project = resolve_existing_project(project)?;
    let (md_path, _) = find_run_by_id(&project.project_root, run_id)?;
    entry_redact_at_path(&md_path, req)
}

pub fn entry_redact_at_path(md_path: &Path, req: RedactRequest) -> Result<AppendOpResult> {
    let reason = validate_body("reason", &req.reason)?;
    append_op(md_path, |run_id, agent, snapshot_hash| {
        let prev = resolve_target_content(agent, req.target_id, &req.target_kind)?;
        let op = AppendOnlyOpRecord {
            op_id: Uuid::new_v4(),
            op_kind: OP_ENTRY_REDACT.into(),
            actor_category: req.actor_category,
            created_at: Utc::now(),
            reason,
            target_id: Some(req.target_id),
            target_kind: Some(req.target_kind.trim().to_string()),
            previous_snapshot_hash: snapshot_hash.to_string(),
            previous_content: Some(prev),
            // Redaction is explicit: new_content is a marker, prior content remains recoverable.
            new_content: Some("[REDACTED]".into()),
            relationship: LedgerRelationship::Redacted,
        };
        agent.append_only_ops.push(op.clone());
        Ok(AppendOpResult {
            run_id,
            op_id: op.op_id,
            op_kind: op.op_kind.clone(),
            relationship: op.relationship.as_str().into(),
            op,
        })
    })
}

/// Current displayed claim text for a checkpoint (latest amend/supersede, or redacted marker).
pub fn current_checkpoint_claim(agent: &AgentRunState, checkpoint_op_id: Uuid) -> String {
    let original = agent
        .checkpoints
        .iter()
        .find(|c| c.op_id == checkpoint_op_id)
        .map(|c| c.summary.clone())
        .unwrap_or_default();
    let mut current = original;
    for op in &agent.append_only_ops {
        if op.target_id != Some(checkpoint_op_id) {
            continue;
        }
        if op.target_kind.as_deref() != Some("checkpoint") {
            continue;
        }
        match op.relationship {
            LedgerRelationship::Amended | LedgerRelationship::Superseded => {
                if let Some(n) = &op.new_content {
                    current = n.clone();
                }
            }
            LedgerRelationship::Redacted => {
                current = "[REDACTED]".into();
            }
            LedgerRelationship::Observation => {}
        }
    }
    current
}

/// Whether a target has an explicit redaction in history (detectable).
pub fn is_redacted(agent: &AgentRunState, target_id: Uuid) -> bool {
    agent.append_only_ops.iter().any(|op| {
        op.target_id == Some(target_id) && op.relationship == LedgerRelationship::Redacted
    })
}

fn checkpoint_summary(agent: &AgentRunState, op_id: Uuid) -> Result<String> {
    agent
        .checkpoints
        .iter()
        .find(|c| c.op_id == op_id)
        .map(|c| c.summary.clone())
        .ok_or_else(|| Error::InvalidFinding {
            message: format!("checkpoint {op_id} is not present on this run"),
        })
}

fn ensure_target_exists(agent: &AgentRunState, target_id: Uuid, kind: &str) -> Result<()> {
    match kind {
        "checkpoint" => {
            checkpoint_summary(agent, target_id)?;
            Ok(())
        }
        "observation" | "amendment" | "supersession" | "redaction" | "append_op" => {
            if agent.append_only_ops.iter().any(|o| o.op_id == target_id) {
                Ok(())
            } else {
                Err(Error::InvalidFinding {
                    message: format!("append-only target {target_id} not found"),
                })
            }
        }
        other => Err(Error::InvalidFinding {
            message: format!("unknown target_kind {other:?}"),
        }),
    }
}

fn resolve_target_content(agent: &AgentRunState, target_id: Uuid, kind: &str) -> Result<String> {
    match kind.trim() {
        "checkpoint" => {
            // Ensure the checkpoint exists on this run (do not silently use "").
            let _ = checkpoint_summary(agent, target_id)?;
            Ok(current_checkpoint_claim(agent, target_id))
        }
        "observation" | "amendment" | "append_op" => agent
            .append_only_ops
            .iter()
            .find(|o| o.op_id == target_id)
            .and_then(|o| o.new_content.clone().or_else(|| o.previous_content.clone()))
            .ok_or_else(|| Error::InvalidFinding {
                message: format!("target op {target_id} has no content to capture"),
            }),
        other => Err(Error::InvalidFinding {
            message: format!("unsupported target_kind for content capture: {other:?}"),
        }),
    }
}

fn validate_body(field: &str, value: &str) -> Result<String> {
    let t = value.trim();
    if t.is_empty() {
        return Err(Error::InvalidFinding {
            message: format!("{field} is required"),
        });
    }
    if t.chars().count() > MAX_OP_BODY_CHARS {
        return Err(Error::InvalidFinding {
            message: format!("{field} exceeds {MAX_OP_BODY_CHARS} characters"),
        });
    }
    for ch in t.chars() {
        if ch == '\0' || (ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t') {
            return Err(Error::InvalidFinding {
                message: format!("{field} must not contain control characters"),
            });
        }
    }
    Ok(t.to_string())
}

fn append_op<F>(md_path: &Path, f: F) -> Result<AppendOpResult>
where
    F: FnOnce(Uuid, &mut AgentRunState, &str) -> Result<AppendOpResult>,
{
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = load_or_migrate_locked(md_path)?;
    let markdown = Document::read_file(md_path)?;
    let snapshot_hash = content_hash(&markdown);
    recover_incomplete_agent_op(md_path, &mut meta, &snapshot_hash)?;
    meta = load_or_migrate_locked(md_path)?;
    let markdown = Document::read_file(md_path)?;
    let snapshot_hash = content_hash(&markdown);
    let run_id = meta.run.id;
    let agent = meta
        .agent
        .as_mut()
        .ok_or_else(|| Error::other("run missing agent state"))?;
    // Capture checkpoints to assert immutability after op.
    let cp_before = agent.checkpoints.clone();
    let result = f(run_id, agent, &snapshot_hash)?;
    if agent.checkpoints != cp_before {
        return Err(Error::other(
            "append-only op must not rewrite checkpoints in place",
        ));
    }
    // Re-project Markdown so original/amendment/current appear without rewriting claims.
    let human = extract_human_notes(&markdown).unwrap_or_default();
    let new_md = render_run_markdown_with_id(run_id, agent, &human);
    meta.touch();
    write_run_meta_unlocked(md_path, &meta)?;
    write_atomic(md_path, new_md.as_bytes())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_protocol::ops::{run_checkpoint, run_start, CheckpointInput, RunStartRequest};
    use crate::agent_protocol::project::init_project;
    use crate::run_meta::load_run_meta_readonly;
    use tempfile::tempdir;

    fn start_with_cp(dir: &Path) -> (Uuid, std::path::PathBuf, Uuid, String) {
        let project = init_project(Some(dir)).unwrap();
        let start = run_start(RunStartRequest {
            objective: "append-only test".into(),
            idempotency_key: "ao-start".into(),
            project: Some(project.project_root.clone()),
            session_id: None,
        })
        .unwrap();
        let cp = run_checkpoint(
            Some(&project.project_root),
            start.run_id,
            &start.content_hash,
            "ao-cp",
            CheckpointInput {
                summary: "All concurrency tests pass.".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
            },
        )
        .unwrap();
        (
            start.run_id,
            project.project_root,
            cp.op_id.unwrap(),
            cp.content_hash,
        )
    }

    #[test]
    fn observation_amend_supersede_redact_persist_and_preserve_checkpoints() {
        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, _) = start_with_cp(dir.path());

        let obs = human_observation_add(
            Some(&root),
            run_id,
            HumanObservationRequest {
                body: "Need to confirm outcome-first ordering coverage.".into(),
                reason: "review note".into(),
                target_id: Some(cp_id),
                target_kind: Some("checkpoint".into()),
            },
        )
        .unwrap();
        assert_eq!(obs.op_kind, OP_HUMAN_OBSERVATION_ADD);
        assert_eq!(obs.relationship, "observation");
        assert_eq!(obs.op.actor_category, ActorCategory::Human);
        assert!(!obs.op.previous_snapshot_hash.is_empty());
        assert_eq!(
            obs.op.new_content.as_deref(),
            Some("Need to confirm outcome-first ordering coverage.")
        );

        let amd = run_amend(
            Some(&root),
            run_id,
            AmendRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "Original claim incomplete".into(),
                new_content: "All concurrency tests, including outcome-first ordering, pass."
                    .into(),
                actor_category: ActorCategory::Agent,
            },
        )
        .unwrap();
        assert_eq!(amd.op_kind, OP_RUN_AMEND);
        assert_eq!(amd.relationship, "amended");
        assert_eq!(
            amd.op.previous_content.as_deref(),
            Some("All concurrency tests pass.")
        );
        assert_eq!(
            amd.op.new_content.as_deref(),
            Some("All concurrency tests, including outcome-first ordering, pass.")
        );

        let sup = entry_supersede(
            Some(&root),
            run_id,
            SupersedeRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "Clarify further".into(),
                new_content: "All concurrency and ordering tests pass under CI.".into(),
                actor_category: ActorCategory::Agent,
            },
        )
        .unwrap();
        assert_eq!(sup.relationship, "superseded");

        let red = entry_redact(
            Some(&root),
            run_id,
            RedactRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "Sensitive wording".into(),
                actor_category: ActorCategory::Human,
            },
        )
        .unwrap();
        assert_eq!(red.relationship, "redacted");
        assert_eq!(red.op.new_content.as_deref(), Some("[REDACTED]"));
        assert!(red.op.previous_content.is_some());

        // Reload
        let md = find_run_by_id(&root, run_id).unwrap().0;
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let agent = meta.agent.as_ref().unwrap();
        assert_eq!(agent.append_only_ops.len(), 4);
        assert_eq!(agent.checkpoints.len(), 1);
        assert_eq!(
            agent.checkpoints[0].summary, "All concurrency tests pass.",
            "checkpoint must remain immutable"
        );
        assert!(is_redacted(agent, cp_id));
        assert_eq!(current_checkpoint_claim(agent, cp_id), "[REDACTED]");
        // Prior content recoverable from redaction op
        let redact_op = agent
            .append_only_ops
            .iter()
            .find(|o| o.op_kind == OP_ENTRY_REDACT)
            .unwrap();
        assert!(
            redact_op
                .previous_content
                .as_ref()
                .unwrap()
                .contains("ordering")
                || redact_op.previous_content.as_ref().unwrap().contains("CI")
        );

        // No approval vocabulary on ops
        let ser = serde_json::to_string(&agent.append_only_ops).unwrap();
        for banned in ["approved", "rejected", "pass_fail", "acceptance"] {
            assert!(!ser.contains(banned), "must not contain {banned}");
        }
    }

    #[test]
    fn markdown_projection_retains_original_and_current_statement() {
        use crate::agent_protocol::markdown::render_run_markdown_with_id;
        use std::fs;

        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, _) = start_with_cp(dir.path());
        run_amend(
            Some(&root),
            run_id,
            AmendRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "The original claim was incomplete. Ordering test missing.".into(),
                new_content: "All concurrency tests, including outcome-first ordering, pass."
                    .into(),
                actor_category: ActorCategory::Agent,
            },
        )
        .unwrap();
        let md_path = find_run_by_id(&root, run_id).unwrap().0;
        let md = fs::read_to_string(&md_path).unwrap();
        assert!(
            md.contains("Original claim:"),
            "projection must keep original: {md}"
        );
        assert!(
            md.contains("All concurrency tests pass."),
            "original summary must remain: {md}"
        );
        assert!(
            md.contains("Current statement:"),
            "projection must show current statement: {md}"
        );
        assert!(
            md.contains("including outcome-first ordering"),
            "current claim must appear: {md}"
        );
        // Structured checkpoints still immutable
        let meta = load_run_meta_readonly(&md_path).unwrap().unwrap();
        let agent = meta.agent.as_ref().unwrap();
        assert_eq!(agent.checkpoints[0].summary, "All concurrency tests pass.");
        // Re-render still consistent
        let again = render_run_markdown_with_id(run_id, agent, "");
        assert!(again.contains("Original claim:"));
    }

    #[test]
    fn rejects_amend_of_unknown_checkpoint() {
        let dir = tempdir().unwrap();
        let (run_id, root, _cp_id, _) = start_with_cp(dir.path());
        let err = run_amend(
            Some(&root),
            run_id,
            AmendRequest {
                target_id: Uuid::new_v4(),
                target_kind: "checkpoint".into(),
                reason: "x".into(),
                new_content: "y".into(),
                actor_category: ActorCategory::Human,
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::InvalidFinding { .. }));
    }

    /// Sequential amend/supersede must freeze the immediately prior claim as previous_content.
    #[test]
    fn sequential_amend_previous_content_is_immediate_prior_claim() {
        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, _) = start_with_cp(dir.path());

        let a1 = run_amend(
            Some(&root),
            run_id,
            AmendRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "first fix".into(),
                new_content: "Claim after first amend.".into(),
                actor_category: ActorCategory::Agent,
            },
        )
        .unwrap();
        assert_eq!(
            a1.op.previous_content.as_deref(),
            Some("All concurrency tests pass."),
            "first amend freezes original summary"
        );
        assert_eq!(
            a1.op.new_content.as_deref(),
            Some("Claim after first amend.")
        );

        let a2 = run_amend(
            Some(&root),
            run_id,
            AmendRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "second fix".into(),
                new_content: "Claim after second amend.".into(),
                actor_category: ActorCategory::Human,
            },
        )
        .unwrap();
        assert_eq!(
            a2.op.previous_content.as_deref(),
            Some("Claim after first amend."),
            "second amend must freeze first amend new_content, not original"
        );
        assert_eq!(
            a2.op.new_content.as_deref(),
            Some("Claim after second amend.")
        );

        // After supersede, next amend freezes the superseding statement.
        let sup = entry_supersede(
            Some(&root),
            run_id,
            SupersedeRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "replace".into(),
                new_content: "Claim after supersede.".into(),
                actor_category: ActorCategory::Agent,
            },
        )
        .unwrap();
        assert_eq!(
            sup.op.previous_content.as_deref(),
            Some("Claim after second amend.")
        );

        let a3 = run_amend(
            Some(&root),
            run_id,
            AmendRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "post-supersede tweak".into(),
                new_content: "Claim after supersede then amend.".into(),
                actor_category: ActorCategory::Agent,
            },
        )
        .unwrap();
        assert_eq!(
            a3.op.previous_content.as_deref(),
            Some("Claim after supersede."),
            "amend after supersede freezes supersede new_content"
        );

        // Original checkpoint record still immutable.
        let md = find_run_by_id(&root, run_id).unwrap().0;
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let agent = meta.agent.as_ref().unwrap();
        assert_eq!(agent.checkpoints[0].summary, "All concurrency tests pass.");
        assert_eq!(
            current_checkpoint_claim(agent, cp_id),
            "Claim after supersede then amend."
        );
    }

    #[test]
    fn required_fields_present_on_each_op_kind() {
        let dir = tempdir().unwrap();
        let (run_id, root, cp_id, _) = start_with_cp(dir.path());
        for (op_kind, relationship) in [
            (OP_HUMAN_OBSERVATION_ADD, "observation"),
            (OP_RUN_AMEND, "amended"),
            (OP_ENTRY_SUPERSEDE, "superseded"),
            (OP_ENTRY_REDACT, "redacted"),
        ] {
            let r = match op_kind {
                OP_HUMAN_OBSERVATION_ADD => human_observation_add(
                    Some(&root),
                    run_id,
                    HumanObservationRequest {
                        body: "obs".into(),
                        reason: "r".into(),
                        target_id: None,
                        target_kind: None,
                    },
                ),
                OP_RUN_AMEND => run_amend(
                    Some(&root),
                    run_id,
                    AmendRequest {
                        target_id: cp_id,
                        target_kind: "checkpoint".into(),
                        reason: "r".into(),
                        new_content: "n".into(),
                        actor_category: ActorCategory::Human,
                    },
                ),
                OP_ENTRY_SUPERSEDE => entry_supersede(
                    Some(&root),
                    run_id,
                    SupersedeRequest {
                        target_id: cp_id,
                        target_kind: "checkpoint".into(),
                        reason: "r".into(),
                        new_content: "n2".into(),
                        actor_category: ActorCategory::Agent,
                    },
                ),
                OP_ENTRY_REDACT => entry_redact(
                    Some(&root),
                    run_id,
                    RedactRequest {
                        target_id: cp_id,
                        target_kind: "checkpoint".into(),
                        reason: "r".into(),
                        actor_category: ActorCategory::Human,
                    },
                ),
                _ => unreachable!(),
            }
            .unwrap();
            assert_eq!(r.op.op_kind, op_kind);
            assert_eq!(r.relationship, relationship);
            assert!(!r.op.op_id.is_nil());
            assert!(!r.op.reason.is_empty());
            assert!(!r.op.previous_snapshot_hash.is_empty());
            assert!(!r.op.created_at.to_rfc3339().is_empty());
        }
    }
}
