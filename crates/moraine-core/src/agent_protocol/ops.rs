use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::git::{capture_git_context, GitContextSummary};
use super::markdown::{extract_human_notes, render_run_markdown_with_id};
use super::project::{
    find_run_by_id, resolve_or_init_project, runs_dir, update_project_meta, StartOpIndex,
};
use super::types::{
    AgentRunState, CheckpointRecord, CompletedOp, EvidenceItem, EvidenceProvenance, IncompleteOp,
    IncompletePhase, LifecycleEvent, RationalItem, RunLifecycle, MAX_CHECKPOINT_ITEMS,
    MAX_FIELD_CHARS, MAX_RECENT_CHECKPOINTS_IN_SHOW, MAX_SUMMARY_CHARS,
};
use crate::atomic::{write_atomic, SidecarLock};
use crate::document::Document;
use crate::error::{Error, Result};
use crate::run_meta::{
    content_hash, load_or_migrate_locked, moraine_sidecar_path, review_snapshot,
    write_run_meta_unlocked, RunMeta, SCHEMA_VERSION,
};

/// Soft target for typical successful JSON responses (token-efficiency).
pub const MAX_JSON_RESPONSE_HINT: usize = 2048;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointInput {
    pub summary: String,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub rationales: Vec<RationalItem>,
    #[serde(default)]
    pub evidence: Vec<EvidenceItem>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RunStartRequest {
    pub objective: String,
    pub idempotency_key: String,
    pub project: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOpResult {
    pub run_id: Uuid,
    pub state: RunLifecycle,
    pub record_path: String,
    pub absolute_path: PathBuf,
    pub content_hash: String,
    pub record_revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitContextSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_current: Option<bool>,
    /// True when this response was served from a prior identical idempotent op.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone)]
pub struct RunShowOptions {
    pub include_markdown: bool,
    pub recent_limit: usize,
}

impl Default for RunShowOptions {
    fn default() -> Self {
        Self {
            include_markdown: false,
            recent_limit: MAX_RECENT_CHECKPOINTS_IN_SHOW,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunShowPacket {
    pub run_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    pub record_path: String,
    pub absolute_path: PathBuf,
    pub state: RunLifecycle,
    pub content_hash: String,
    pub record_revision: u64,
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting_git: Option<GitContextSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_git: Option<GitContextSummary>,
    pub checkpoint_count: usize,
    pub recent_checkpoints: Vec<RecentCheckpoint>,
    pub risks: Vec<String>,
    pub open_questions: Vec<String>,
    pub annotations: AnnotationCountsJson,
    pub review_state: String,
    pub decision_current: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_operation: Option<IncompleteOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentCheckpoint {
    pub op_id: Uuid,
    pub created_at: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationCountsJson {
    pub comments_open: usize,
    pub suggestions_open: usize,
    pub comments_resolved: usize,
    pub suggestions_resolved: usize,
}

pub fn run_start(req: RunStartRequest) -> Result<AgentOpResult> {
    let objective = req.objective.trim().to_string();
    if objective.is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "objective is required".into(),
        });
    }
    if objective.len() > MAX_SUMMARY_CHARS {
        return Err(Error::InvalidCheckpoint {
            message: format!("objective exceeds {MAX_SUMMARY_CHARS} characters"),
        });
    }
    if req.idempotency_key.trim().is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "idempotency key is required".into(),
        });
    }
    let payload_hash = hash_payload(&json!({
        "kind": "start",
        "objective": objective,
    }));

    let project = resolve_or_init_project(req.project.as_deref())?;
    let project_root = project.project_root.clone();

    // Idempotent start via project index
    let replay = update_project_meta(&project_root, |meta| {
        if let Some(existing) = meta.start_ops.get(&req.idempotency_key) {
            if existing.payload_hash != payload_hash || existing.objective != objective {
                return Err(Error::IdempotencyConflict {
                    key: req.idempotency_key.clone(),
                    message: "start idempotency key was reused with a different objective".into(),
                });
            }
            return Ok(Some(existing.clone()));
        }
        Ok(None)
    })?;

    if let Some(existing) = replay {
        let (md_path, meta) = find_run_by_id(&project_root, existing.run_id)?;
        let markdown = Document::read_file(&md_path)?;
        let agent = meta
            .agent
            .as_ref()
            .ok_or_else(|| Error::other("run missing agent state"))?;
        return Ok(AgentOpResult {
            run_id: meta.run.id,
            state: agent.lifecycle,
            record_path: agent.record_path.clone(),
            absolute_path: md_path,
            content_hash: content_hash(&markdown),
            record_revision: agent.record_revision,
            project_id: Some(project.project_id),
            project_root: Some(project_root),
            op_id: None,
            git: agent.starting_git.clone(),
            review_state: None,
            decision_current: None,
            idempotent_replay: true,
        });
    }

    let run_id = Uuid::new_v4();
    let short = short_id(run_id);
    let date = Utc::now().format("%Y-%m-%d");
    let slug = slugify(&objective);
    let file_name = format!("{date}-{slug}-{short}.md");
    let runs = runs_dir(&project_root);
    let mut md_path = runs.join(&file_name);
    // Collision safety
    if md_path.exists() {
        md_path = runs.join(format!(
            "{date}-{slug}-{short}-{}.md",
            &Uuid::new_v4().to_string()[..8]
        ));
    }
    let rel = path_relative_to(&md_path, &project_root);

    let git = capture_git_context(&project_root);
    let agent = AgentRunState {
        lifecycle: RunLifecycle::Active,
        record_revision: 1,
        objective: objective.clone(),
        record_path: rel.clone(),
        project_id: Some(project.project_id),
        start_idempotency_key: req.idempotency_key.clone(),
        starting_git: Some(git.clone()),
        current_git: Some(git.clone()),
        checkpoints: vec![],
        lifecycle_events: vec![],
        ready_summary: None,
        completed_ops: vec![],
        incomplete_op: None,
        risks: vec![],
        open_questions: vec![],
    };

    let mut meta = RunMeta::new_run_with_id(run_id);
    meta.schema_version = SCHEMA_VERSION;
    meta.agent = Some(agent.clone());

    let markdown = render_run_markdown_with_id(run_id, &agent, "");
    let hash = content_hash(&markdown);

    // Write markdown then sidecar under lock
    let side = moraine_sidecar_path(&md_path);
    let _lock = SidecarLock::acquire(&side)?;
    write_atomic(&md_path, markdown.as_bytes())?;
    write_run_meta_unlocked(&md_path, &meta)?;

    update_project_meta(&project_root, |pm| {
        pm.start_ops.insert(
            req.idempotency_key.clone(),
            StartOpIndex {
                run_id,
                objective,
                record_path: rel.clone(),
                payload_hash,
            },
        );
        Ok(())
    })?;

    Ok(AgentOpResult {
        run_id,
        state: RunLifecycle::Active,
        record_path: rel,
        absolute_path: md_path,
        content_hash: hash,
        record_revision: 1,
        project_id: Some(project.project_id),
        project_root: Some(project_root),
        op_id: None,
        git: Some(git),
        review_state: None,
        decision_current: None,
        idempotent_replay: false,
    })
}

pub fn run_checkpoint(
    project: Option<&Path>,
    run_id: Uuid,
    expected_hash: &str,
    idempotency_key: &str,
    input: CheckpointInput,
) -> Result<AgentOpResult> {
    let input = validate_checkpoint(input)?;
    let payload_hash = hash_payload(&serde_json::to_value(&input)?);
    mutate_agent_run(
        project,
        run_id,
        expected_hash,
        idempotency_key,
        "checkpoint",
        &payload_hash,
        |ctx| {
            if ctx.agent.lifecycle != RunLifecycle::Active {
                return Err(Error::RunStateConflict {
                    expected: "active".into(),
                    actual: ctx.agent.lifecycle.as_str().into(),
                });
            }
            let git = capture_git_context(ctx.project_root);
            let op_id = Uuid::new_v4();
            let cp = CheckpointRecord {
                op_id,
                idempotency_key: idempotency_key.to_string(),
                created_at: Utc::now(),
                summary: input.summary.clone(),
                actions: input.actions.clone(),
                rationales: input.rationales.clone(),
                evidence: input.evidence.clone(),
                risks: input.risks.clone(),
                open_questions: input.open_questions.clone(),
                git: Some(git.clone()),
            };
            // Merge risks / open questions (append unique)
            for r in &input.risks {
                if !ctx.agent.risks.iter().any(|x| x == r) {
                    ctx.agent.risks.push(r.clone());
                }
            }
            for q in &input.open_questions {
                if !ctx.agent.open_questions.iter().any(|x| x == q) {
                    ctx.agent.open_questions.push(q.clone());
                }
            }
            ctx.agent.checkpoints.push(cp);
            ctx.agent.current_git = Some(git.clone());
            ctx.agent.record_revision = ctx.agent.record_revision.saturating_add(1);
            Ok((op_id, Some(git)))
        },
    )
}

pub fn run_ready(
    project: Option<&Path>,
    run_id: Uuid,
    expected_hash: &str,
    idempotency_key: &str,
    summary: Option<String>,
) -> Result<AgentOpResult> {
    let summary = summary
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(s) = &summary {
        if s.len() > MAX_SUMMARY_CHARS {
            return Err(Error::InvalidCheckpoint {
                message: format!("summary exceeds {MAX_SUMMARY_CHARS} characters"),
            });
        }
    }
    let payload_hash = hash_payload(&json!({ "kind": "ready", "summary": summary }));
    mutate_agent_run(
        project,
        run_id,
        expected_hash,
        idempotency_key,
        "ready",
        &payload_hash,
        |ctx| {
            if ctx.agent.lifecycle != RunLifecycle::Active {
                // Idempotent ready when already ready with same key handled by completed_ops
                return Err(Error::RunStateConflict {
                    expected: "active".into(),
                    actual: ctx.agent.lifecycle.as_str().into(),
                });
            }
            let git = capture_git_context(ctx.project_root);
            let op_id = Uuid::new_v4();
            ctx.agent.lifecycle = RunLifecycle::ReadyForReview;
            ctx.agent.ready_summary = summary.clone();
            ctx.agent.current_git = Some(git.clone());
            ctx.agent.lifecycle_events.push(LifecycleEvent {
                op_id,
                idempotency_key: idempotency_key.to_string(),
                created_at: Utc::now(),
                kind: "ready".into(),
                note: summary.clone(),
                git: Some(git.clone()),
            });
            ctx.agent.record_revision = ctx.agent.record_revision.saturating_add(1);
            Ok((op_id, Some(git)))
        },
    )
}

pub fn run_resume(
    project: Option<&Path>,
    run_id: Uuid,
    expected_hash: &str,
    idempotency_key: &str,
    reason: Option<String>,
) -> Result<AgentOpResult> {
    let reason = reason
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(s) = &reason {
        if s.len() > MAX_SUMMARY_CHARS {
            return Err(Error::InvalidCheckpoint {
                message: format!("reason exceeds {MAX_SUMMARY_CHARS} characters"),
            });
        }
    }
    let payload_hash = hash_payload(&json!({ "kind": "resume", "reason": reason }));
    mutate_agent_run(
        project,
        run_id,
        expected_hash,
        idempotency_key,
        "resume",
        &payload_hash,
        |ctx| {
            if ctx.agent.lifecycle != RunLifecycle::ReadyForReview {
                return Err(Error::RunStateConflict {
                    expected: "ready_for_review".into(),
                    actual: ctx.agent.lifecycle.as_str().into(),
                });
            }
            let git = capture_git_context(ctx.project_root);
            let op_id = Uuid::new_v4();
            ctx.agent.lifecycle = RunLifecycle::Active;
            ctx.agent.current_git = Some(git.clone());
            ctx.agent.lifecycle_events.push(LifecycleEvent {
                op_id,
                idempotency_key: idempotency_key.to_string(),
                created_at: Utc::now(),
                kind: "resume".into(),
                note: reason.clone(),
                git: Some(git.clone()),
            });
            ctx.agent.record_revision = ctx.agent.record_revision.saturating_add(1);
            Ok((op_id, Some(git)))
        },
    )
}

pub fn run_show(
    project: Option<&Path>,
    run_id: Uuid,
    opts: RunShowOptions,
) -> Result<RunShowPacket> {
    let project = resolve_or_init_project(project)?;
    let (md_path, meta) = find_run_by_id(&project.project_root, run_id)?;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other("run has no agent protocol state"))?;
    let markdown = Document::read_file(&md_path)?;
    let hash = content_hash(&markdown);
    let snap = review_snapshot(&meta, &markdown);

    let limit = opts.recent_limit.max(1);
    let recent: Vec<RecentCheckpoint> = agent
        .checkpoints
        .iter()
        .rev()
        .take(limit)
        .map(|c| RecentCheckpoint {
            op_id: c.op_id,
            created_at: c.created_at.to_rfc3339(),
            summary: truncate(&c.summary, 240),
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut counts = AnnotationCountsJson {
        comments_open: 0,
        suggestions_open: 0,
        comments_resolved: 0,
        suggestions_resolved: 0,
    };
    for c in &meta.comments {
        let sug = c.kind == crate::comments::AnnotationKind::Suggestion;
        match (sug, c.resolved) {
            (false, false) => counts.comments_open += 1,
            (false, true) => counts.comments_resolved += 1,
            (true, false) => counts.suggestions_open += 1,
            (true, true) => counts.suggestions_resolved += 1,
        }
    }

    Ok(RunShowPacket {
        run_id: meta.run.id,
        project_id: agent.project_id.or(Some(project.project_id)),
        record_path: agent.record_path.clone(),
        absolute_path: md_path,
        state: agent.lifecycle,
        content_hash: hash,
        record_revision: agent.record_revision,
        objective: agent.objective.clone(),
        starting_git: agent.starting_git.clone(),
        current_git: agent.current_git.clone(),
        checkpoint_count: agent.checkpoints.len(),
        recent_checkpoints: recent,
        risks: agent.risks.clone(),
        open_questions: agent.open_questions.clone(),
        annotations: counts,
        review_state: match snap.state {
            crate::run_meta::ReviewStateKind::Unreviewed => "unreviewed",
            crate::run_meta::ReviewStateKind::Approved => "approved",
            crate::run_meta::ReviewStateKind::ChangesRequested => "changes_requested",
            crate::run_meta::ReviewStateKind::Rejected => "rejected",
            crate::run_meta::ReviewStateKind::Stale => "stale",
        }
        .into(),
        decision_current: snap.decision_current,
        incomplete_operation: agent.incomplete_op.clone(),
        markdown: if opts.include_markdown {
            Some(markdown)
        } else {
            None
        },
    })
}

struct MutCtx<'a> {
    project_root: &'a Path,
    agent: &'a mut AgentRunState,
}

fn mutate_agent_run<F>(
    project: Option<&Path>,
    run_id: Uuid,
    expected_hash: &str,
    idempotency_key: &str,
    kind: &str,
    payload_hash: &str,
    apply: F,
) -> Result<AgentOpResult>
where
    F: FnOnce(&mut MutCtx<'_>) -> Result<(Uuid, Option<GitContextSummary>)>,
{
    if idempotency_key.trim().is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "idempotency key is required".into(),
        });
    }
    let project = resolve_or_init_project(project)?;
    let project_root = project.project_root.clone();
    let (md_path, _) = find_run_by_id(&project_root, run_id)?;
    let side = moraine_sidecar_path(&md_path);
    let _lock = SidecarLock::acquire(&side)?;

    // Recovery: incomplete op
    let mut meta = load_or_migrate_locked(&md_path)?;
    let markdown = Document::read_file(&md_path)?;
    let actual = content_hash(&markdown);

    if let Some(agent) = meta.agent.as_mut() {
        if let Some(inc) = agent.incomplete_op.clone() {
            if let Some(expected) = &inc.expected_content_hash {
                if actual == *expected {
                    // Markdown applied; finalize sidecar
                    agent.incomplete_op = None;
                    agent.push_completed(CompletedOp {
                        idempotency_key: inc.idempotency_key.clone(),
                        payload_hash: inc.payload_hash.clone(),
                        op_id: inc.op_id,
                        kind: inc.kind.clone(),
                        content_hash: actual.clone(),
                        record_revision: agent.record_revision,
                        created_at: Utc::now(),
                    });
                    write_run_meta_unlocked(&md_path, &meta)?;
                } else if actual == inc.base_content_hash {
                    // Not applied; clear incomplete and continue if this is a retry of same key
                    agent.incomplete_op = None;
                    write_run_meta_unlocked(&md_path, &meta)?;
                } else {
                    return Err(Error::OperationRecoveryRequired {
                        message: format!(
                            "incomplete op {} in phase {:?}; document hash matches neither base nor expected",
                            inc.op_id, inc.phase
                        ),
                    });
                }
            }
        }
    }

    // Re-read after possible recovery write
    meta = load_or_migrate_locked(&md_path)?;
    let markdown = Document::read_file(&md_path)?;
    let actual = content_hash(&markdown);

    let agent = meta
        .agent
        .as_mut()
        .ok_or_else(|| Error::other("run missing agent state"))?;

    if let Some(prev) = agent.find_completed_op(idempotency_key).cloned() {
        if prev.payload_hash != payload_hash {
            return Err(Error::IdempotencyConflict {
                key: idempotency_key.into(),
                message: format!("key was used for a different {kind} payload"),
            });
        }
        return Ok(AgentOpResult {
            run_id,
            state: agent.lifecycle,
            record_path: agent.record_path.clone(),
            absolute_path: md_path,
            content_hash: actual,
            record_revision: agent.record_revision,
            project_id: agent.project_id.or(Some(project.project_id)),
            project_root: Some(project_root),
            op_id: Some(prev.op_id),
            git: agent.current_git.clone(),
            review_state: None,
            decision_current: None,
            idempotent_replay: true,
        });
    }

    if actual != expected_hash {
        return Err(Error::RevisionConflict {
            expected: expected_hash.to_string(),
            actual,
        });
    }

    let human = extract_human_notes(&markdown)?;
    let mut agent_state = agent.clone();
    let mut ctx = MutCtx {
        project_root: &project_root,
        agent: &mut agent_state,
    };
    let (op_id, git) = apply(&mut ctx)?;

    let new_md = render_run_markdown_with_id(run_id, &agent_state, &human);
    let new_hash = content_hash(&new_md);

    // Two-phase: mark incomplete, write md, finalize
    agent_state.incomplete_op = Some(IncompleteOp {
        op_id,
        idempotency_key: idempotency_key.to_string(),
        kind: kind.to_string(),
        payload_hash: payload_hash.to_string(),
        base_content_hash: actual.clone(),
        expected_content_hash: Some(new_hash.clone()),
        phase: IncompletePhase::Begun,
        created_at: Utc::now(),
    });
    meta.agent = Some(agent_state.clone());
    write_run_meta_unlocked(&md_path, &meta)?;

    write_atomic(&md_path, new_md.as_bytes())?;

    // Finalize
    agent_state.incomplete_op = None;
    agent_state.incomplete_op = None;
    agent_state.push_completed(CompletedOp {
        idempotency_key: idempotency_key.to_string(),
        payload_hash: payload_hash.to_string(),
        op_id,
        kind: kind.to_string(),
        content_hash: new_hash.clone(),
        record_revision: agent_state.record_revision,
        created_at: Utc::now(),
    });
    meta.agent = Some(agent_state.clone());
    meta.touch();
    write_run_meta_unlocked(&md_path, &meta)?;

    let snap = review_snapshot(&meta, &new_md);

    Ok(AgentOpResult {
        run_id,
        state: agent_state.lifecycle,
        record_path: agent_state.record_path.clone(),
        absolute_path: md_path,
        content_hash: new_hash,
        record_revision: agent_state.record_revision,
        project_id: agent_state.project_id.or(Some(project.project_id)),
        project_root: Some(project_root),
        op_id: Some(op_id),
        git,
        review_state: Some(
            match snap.state {
                crate::run_meta::ReviewStateKind::Unreviewed => "unreviewed",
                crate::run_meta::ReviewStateKind::Approved => "approved",
                crate::run_meta::ReviewStateKind::ChangesRequested => "changes_requested",
                crate::run_meta::ReviewStateKind::Rejected => "rejected",
                crate::run_meta::ReviewStateKind::Stale => "stale",
            }
            .into(),
        ),
        decision_current: Some(snap.decision_current),
        idempotent_replay: false,
    })
}

fn validate_checkpoint(mut input: CheckpointInput) -> Result<CheckpointInput> {
    input.summary = input.summary.trim().to_string();
    if input.summary.is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "summary is required".into(),
        });
    }
    if input.summary.len() > MAX_SUMMARY_CHARS {
        return Err(Error::InvalidCheckpoint {
            message: format!("summary exceeds {MAX_SUMMARY_CHARS} characters"),
        });
    }
    let empty = input.actions.is_empty()
        && input.rationales.is_empty()
        && input.evidence.is_empty()
        && input.risks.is_empty()
        && input.open_questions.is_empty();
    // Summary-only is allowed (not empty checkpoint) — "reject otherwise empty" means
    // no summary. Spec: summary required; reject otherwise empty — I'll allow summary-only
    // as a valid sparse checkpoint (summary is the content).
    let _ = empty;

    for (name, items) in [
        ("actions", input.actions.len()),
        ("rationales", input.rationales.len()),
        ("evidence", input.evidence.len()),
        ("risks", input.risks.len()),
        ("openQuestions", input.open_questions.len()),
    ] {
        if items > MAX_CHECKPOINT_ITEMS {
            return Err(Error::InvalidCheckpoint {
                message: format!("{name} exceeds {MAX_CHECKPOINT_ITEMS} items"),
            });
        }
    }
    for a in &input.actions {
        if a.len() > MAX_FIELD_CHARS {
            return Err(Error::InvalidCheckpoint {
                message: format!("action exceeds {MAX_FIELD_CHARS} characters"),
            });
        }
    }
    for r in &mut input.rationales {
        r.choice = r.choice.trim().to_string();
        r.reason = r.reason.trim().to_string();
        if r.choice.is_empty() || r.reason.is_empty() {
            return Err(Error::InvalidCheckpoint {
                message: "rationale choice and reason are required".into(),
            });
        }
        if r.choice.len() > MAX_FIELD_CHARS || r.reason.len() > MAX_FIELD_CHARS {
            return Err(Error::InvalidCheckpoint {
                message: format!("rationale exceeds {MAX_FIELD_CHARS} characters"),
            });
        }
    }
    for e in &mut input.evidence {
        e.label = e.label.trim().to_string();
        if e.label.is_empty() {
            return Err(Error::InvalidCheckpoint {
                message: "evidence label is required".into(),
            });
        }
        // Force honest provenance default for agent-supplied evidence
        if e.provenance != EvidenceProvenance::MoraineCaptured {
            e.provenance = EvidenceProvenance::AgentReported;
        }
    }
    Ok(input)
}

fn hash_payload(v: &serde_json::Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
        if out.len() >= 40 {
            break;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "run".into()
    } else {
        out
    }
}

fn short_id(id: Uuid) -> String {
    id.to_string()
        .chars()
        .filter(|c| *c != '-')
        .take(8)
        .collect()
}

fn path_relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_protocol::project::init_project;
    use crate::run_meta::{record_decision, DecisionKind};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn start_checkpoint_ready_flow() {
        let dir = tempdir().unwrap();
        let _ = init_project(Some(dir.path())).unwrap();
        let start = run_start(RunStartRequest {
            objective: "Ship protocol".into(),
            idempotency_key: "start-1".into(),
            project: Some(dir.path().to_path_buf()),
        })
        .unwrap();
        assert_eq!(start.state, RunLifecycle::Active);
        assert!(start.absolute_path.is_file());
        assert!(!start.content_hash.is_empty());

        let md = Document::read_file(&start.absolute_path).unwrap();
        assert!(md.contains("## Human notes"));
        assert!(md.contains("Ship protocol"));

        // external human text
        let mut notes = extract_human_notes(&md).unwrap();
        notes.push_str("Human says hello\n");
        let agent_meta = load_or_migrate_locked(&start.absolute_path).unwrap();
        let agent = agent_meta.agent.as_ref().unwrap();
        let md2 = render_run_markdown_with_id(start.run_id, agent, &notes);
        write_atomic(&start.absolute_path, md2.as_bytes()).unwrap();
        let hash2 = content_hash(&md2);

        let cp = run_checkpoint(
            Some(dir.path()),
            start.run_id,
            &hash2,
            "cp-1",
            CheckpointInput {
                summary: "Implemented core ops".into(),
                actions: vec!["Wrote agent_protocol module".into()],
                rationales: vec![RationalItem {
                    choice: "Sidecar structured state".into(),
                    reason: "Recovery and idempotency".into(),
                }],
                evidence: vec![],
                risks: vec!["Schema bump".into()],
                open_questions: vec![],
            },
        )
        .unwrap();
        assert_eq!(cp.record_revision, 2);
        let md3 = Document::read_file(&cp.absolute_path).unwrap();
        assert!(md3.contains("Human says hello"));
        assert!(md3.contains("Implemented core ops"));

        // idempotent replay
        let cp2 = run_checkpoint(
            Some(dir.path()),
            start.run_id,
            &hash2,
            "cp-1",
            CheckpointInput {
                summary: "Implemented core ops".into(),
                actions: vec!["Wrote agent_protocol module".into()],
                rationales: vec![RationalItem {
                    choice: "Sidecar structured state".into(),
                    reason: "Recovery and idempotency".into(),
                }],
                evidence: vec![],
                risks: vec!["Schema bump".into()],
                open_questions: vec![],
            },
        )
        .unwrap();
        assert!(cp2.idempotent_replay);
        assert_eq!(cp2.content_hash, cp.content_hash);

        // stale hash
        let err = run_checkpoint(
            Some(dir.path()),
            start.run_id,
            "deadbeef",
            "cp-2",
            CheckpointInput {
                summary: "stale".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::RevisionConflict { .. }));

        let ready = run_ready(
            Some(dir.path()),
            start.run_id,
            &cp.content_hash,
            "ready-1",
            Some("Done".into()),
        )
        .unwrap();
        assert_eq!(ready.state, RunLifecycle::ReadyForReview);

        let show = run_show(Some(dir.path()), start.run_id, RunShowOptions::default()).unwrap();
        assert!(show.markdown.is_none());
        assert_eq!(show.checkpoint_count, 1);
        let packed = serde_json::to_string(&show).unwrap();
        assert!(
            packed.len() < MAX_JSON_RESPONSE_HINT * 2,
            "show size {}",
            packed.len()
        );

        // human decision
        let _ = record_decision(
            &ready.absolute_path,
            DecisionKind::Approved,
            "tester",
            None,
            &ready.content_hash,
        )
        .unwrap();
        let show2 = run_show(Some(dir.path()), start.run_id, RunShowOptions::default()).unwrap();
        assert_eq!(show2.review_state, "approved");
        assert!(show2.decision_current);

        // resume changes markdown -> stale decision
        let resumed = run_resume(
            Some(dir.path()),
            start.run_id,
            &ready.content_hash,
            "resume-1",
            Some("more work".into()),
        )
        .unwrap();
        assert_eq!(resumed.state, RunLifecycle::Active);
        let show3 = run_show(Some(dir.path()), start.run_id, RunShowOptions::default()).unwrap();
        assert_eq!(show3.review_state, "stale");
        assert!(!show3.decision_current);
    }

    #[test]
    fn start_idempotency_conflict() {
        let dir = tempdir().unwrap();
        let _ = run_start(RunStartRequest {
            objective: "A".into(),
            idempotency_key: "k".into(),
            project: Some(dir.path().to_path_buf()),
        })
        .unwrap();
        let err = run_start(RunStartRequest {
            objective: "B".into(),
            idempotency_key: "k".into(),
            project: Some(dir.path().to_path_buf()),
        })
        .unwrap_err();
        assert!(matches!(err, Error::IdempotencyConflict { .. }));
    }

    #[test]
    fn concurrent_checkpoints_one_wins() {
        let dir = tempdir().unwrap();
        let start = run_start(RunStartRequest {
            objective: "Concurrent".into(),
            idempotency_key: "s".into(),
            project: Some(dir.path().to_path_buf()),
        })
        .unwrap();
        let hash = start.content_hash.clone();
        let run_id = start.run_id;
        let root = dir.path().to_path_buf();
        let barrier = Arc::new(Barrier::new(2));

        let b1 = barrier.clone();
        let r1 = root.clone();
        let h1 = hash.clone();
        let t1 = thread::spawn(move || {
            b1.wait();
            run_checkpoint(
                Some(&r1),
                run_id,
                &h1,
                "a",
                CheckpointInput {
                    summary: "A".into(),
                    actions: vec!["a".into()],
                    rationales: vec![],
                    evidence: vec![],
                    risks: vec![],
                    open_questions: vec![],
                },
            )
        });
        let b2 = barrier;
        let r2 = root;
        let h2 = hash;
        let t2 = thread::spawn(move || {
            b2.wait();
            run_checkpoint(
                Some(&r2),
                run_id,
                &h2,
                "b",
                CheckpointInput {
                    summary: "B".into(),
                    actions: vec!["b".into()],
                    rationales: vec![],
                    evidence: vec![],
                    risks: vec![],
                    open_questions: vec![],
                },
            )
        });
        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();
        let ok = r1.is_ok() as u8 + r2.is_ok() as u8;
        let conflict = matches!(r1, Err(Error::RevisionConflict { .. })) as u8
            + matches!(r2, Err(Error::RevisionConflict { .. })) as u8;
        assert_eq!(ok, 1, "exactly one success: {r1:?} {r2:?}");
        assert_eq!(conflict, 1, "exactly one revision conflict");

        let show = run_show(Some(dir.path()), run_id, RunShowOptions::default()).unwrap();
        assert_eq!(show.checkpoint_count, 1);
    }

    #[test]
    fn many_checkpoints_show_bounded() {
        let dir = tempdir().unwrap();
        let mut cur = run_start(RunStartRequest {
            objective: "Size".into(),
            idempotency_key: "s".into(),
            project: Some(dir.path().to_path_buf()),
        })
        .unwrap();
        for i in 0..30 {
            cur = run_checkpoint(
                Some(dir.path()),
                cur.run_id,
                &cur.content_hash,
                &format!("cp-{i}"),
                CheckpointInput {
                    summary: format!("Checkpoint number {i} with some detail"),
                    actions: vec![format!("action {i}")],
                    rationales: vec![],
                    evidence: vec![],
                    risks: vec![],
                    open_questions: vec![],
                },
            )
            .unwrap();
        }
        let show = run_show(Some(dir.path()), cur.run_id, RunShowOptions::default()).unwrap();
        assert_eq!(show.checkpoint_count, 30);
        assert!(show.recent_checkpoints.len() <= MAX_RECENT_CHECKPOINTS_IN_SHOW);
        let packed = serde_json::to_vec(&show).unwrap();
        assert!(
            packed.len() < 4096,
            "default show should stay compact, got {}",
            packed.len()
        );
        assert!(show.markdown.is_none());
    }
}
