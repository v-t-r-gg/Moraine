use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::git::{capture_git_context, GitContextSummary};
use super::markdown::{extract_human_notes, render_run_markdown_with_id};
use super::project::{
    find_run_by_id, resolve_existing_project, resolve_or_init_project, runs_dir,
    update_project_meta, StartOpIndex, StartOpStatus,
};
use super::session::{
    derive_capture_coverage, load_session, namespace_session_key, register_session_run,
    session_observe, set_session_provisional_run, SessionObserveRequest,
};
use super::types::{
    AgentRunState, CheckpointRecord, EvidenceItem, EvidenceProvenance, IdempotencyRecord,
    IncompleteOp, IncompletePhase, LifecycleEvent, RationalItem, RunLifecycle,
    MAX_CHECKPOINT_ITEMS, MAX_FIELD_CHARS, MAX_RECENT_CHECKPOINTS_IN_SHOW, MAX_RECENT_LIST_IN_SHOW,
    MAX_SUMMARY_CHARS,
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
    /// When set, confirm a provisional run bound to this session instead of creating a duplicate.
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProvisionalRunRequest {
    pub session_id: String,
    pub project: Option<PathBuf>,
    /// Bounded objective from the first substantive prompt; optional fallback applied.
    pub objective: Option<String>,
    pub idempotency_key: Option<String>,
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
pub struct BoundedStringList {
    pub total: usize,
    pub recent: Vec<String>,
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
    pub risks: BoundedStringList,
    pub open_questions: BoundedStringList,
    pub annotations: AnnotationCountsJson,
    pub review_state: String,
    pub decision_current: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_operation: Option<IncompleteOpSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

/// Compact incomplete-op view without embedding full pending agent state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncompleteOpSummary {
    pub op_id: Uuid,
    pub kind: String,
    pub phase: IncompletePhase,
    pub base_content_hash: String,
    pub expected_content_hash: String,
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
    let objective = require_safe_scalar("objective", req.objective.trim(), MAX_SUMMARY_CHARS)?;
    if objective.is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "objective is required".into(),
        });
    }
    if req.idempotency_key.trim().is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "idempotency key is required".into(),
        });
    }

    // Prefer confirming an *active provisional* run for this session.
    // A second explicit run_start after confirm creates a new run (does not
    // reuse the first confirmed run unless the start idempotency key matches).
    if let Some(external_sid) = req
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let project = resolve_or_init_project(req.project.as_deref())?;
        // Session keys are namespaced; try common integrations when looking up.
        let session_key = namespace_session_key("codex", project.project_id, external_sid);
        let session = match load_session(&project.project_root, &session_key)? {
            Some(s) => Some(s),
            None => {
                // Fall back: observe without confine so MCP can bind a session.
                let obs = session_observe(SessionObserveRequest {
                    session_id: external_sid.to_string(),
                    integration: "codex".into(),
                    project: Some(project.project_root.clone()),
                    source: "mcp_run_start".into(),
                    initial_task: Some(objective.clone()),
                    ended: false,
                    confine_existing_project: false,
                })?;
                load_session(&project.project_root, &obs.session_key)?
            }
        };
        if let Some(session) = session {
            if let Some(run_id) = session.active_provisional_run_id {
                if let Ok((md_path, meta)) = find_run_by_id(&project.project_root, run_id) {
                    if let Some(agent) = meta.agent.as_ref() {
                        if agent.provisional {
                            return confirm_provisional_run(
                                &project.project_root,
                                project.project_id,
                                &session.session_key,
                                md_path,
                                meta,
                                &objective,
                                &req.idempotency_key,
                            );
                        }
                    }
                }
            }
            // Idempotent replay against any prior run in this session with same start key.
            for run_id in &session.run_ids {
                if let Ok((md_path, meta)) = find_run_by_id(&project.project_root, *run_id) {
                    if let Some(agent) = meta.agent.as_ref() {
                        if agent.start_idempotency_key == req.idempotency_key {
                            let markdown = Document::read_file(&md_path)?;
                            return Ok(AgentOpResult {
                                run_id: meta.run.id,
                                state: agent.lifecycle,
                                record_path: agent.record_path.clone(),
                                absolute_path: md_path,
                                content_hash: content_hash(&markdown),
                                record_revision: agent.record_revision,
                                project_id: Some(project.project_id),
                                project_root: Some(project.project_root),
                                op_id: None,
                                git: agent.starting_git.clone(),
                                review_state: None,
                                decision_current: None,
                                idempotent_replay: true,
                            });
                        }
                    }
                }
            }
            // No provisional to confirm and no matching start key → fall through
            // to create a *new* run in this session.
        }
    }

    let payload_hash = hash_payload(&json!({
        "kind": "start",
        "objective": objective,
    }));

    let project = resolve_or_init_project(req.project.as_deref())?;
    let project_root = project.project_root.clone();

    // Reserve under project lock before creating files.
    let reservation = update_project_meta(&project_root, |meta| {
        if let Some(existing) = meta.start_ops.get(&req.idempotency_key).cloned() {
            if existing.payload_hash != payload_hash || existing.objective != objective {
                return Err(Error::IdempotencyConflict {
                    key: req.idempotency_key.clone(),
                    message: "start idempotency key was reused with a different objective".into(),
                });
            }
            return Ok(existing);
        }
        let run_id = Uuid::new_v4();
        let short = short_id(run_id);
        let date = Utc::now().format("%Y-%m-%d");
        let slug = slugify(&objective);
        let mut file_name = format!("{date}-{slug}-{short}.md");
        let runs = runs_dir(&project_root);
        let mut md_path = runs.join(&file_name);
        if md_path.exists() {
            file_name = format!(
                "{date}-{slug}-{short}-{}.md",
                &Uuid::new_v4().to_string()[..8]
            );
            md_path = runs.join(&file_name);
        }
        let rel = path_relative_to(&md_path, &project_root);
        let entry = StartOpIndex {
            run_id,
            objective: objective.clone(),
            record_path: rel,
            payload_hash: payload_hash.clone(),
            status: StartOpStatus::Pending,
        };
        meta.start_ops
            .insert(req.idempotency_key.clone(), entry.clone());
        Ok(entry)
    })?;

    let md_path = project_root.join(&reservation.record_path);
    let rel = reservation.record_path.clone();
    let run_id = reservation.run_id;

    // Idempotent complete: files already exist with agent state.
    if reservation.status == StartOpStatus::Complete && md_path.is_file() {
        if let Some(meta) = crate::run_meta::load_run_meta_readonly(&md_path)? {
            if let Some(agent) = meta.agent.as_ref() {
                let markdown = Document::read_file(&md_path)?;
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
        }
    }

    // Pending or incomplete create: build files with reserved identity.
    let git = capture_git_context(&project_root);
    let external_sid = req
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let session_key = external_sid
        .as_ref()
        .map(|ext| namespace_session_key("codex", project.project_id, ext));
    let session = session_key
        .as_ref()
        .and_then(|k| load_session(&project_root, k).ok().flatten());
    let coverage = derive_capture_coverage(false, session.as_ref(), 0);
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
        idempotency: Default::default(),
        incomplete_op: None,
        risks: vec![],
        open_questions: vec![],
        capture_coverage: coverage,
        session_id: session_key.clone(),
        provisional: false,
        evidence: vec![],
        findings: vec![],
        finding_events: vec![],
        append_only_ops: vec![],
    };

    let mut meta = RunMeta::new_run_with_id(run_id);
    meta.schema_version = SCHEMA_VERSION;
    meta.agent = Some(agent.clone());

    let markdown = render_run_markdown_with_id(run_id, &agent, "");
    let hash = content_hash(&markdown);

    let side = moraine_sidecar_path(&md_path);
    let _lock = SidecarLock::acquire(&side)?;
    // If files already exist from a prior crash after write, do not overwrite content
    // when the run id matches (recoverable pending).
    if md_path.is_file() {
        if let Some(existing) = crate::run_meta::load_run_meta_readonly(&md_path)? {
            if existing.run.id == run_id {
                let markdown = Document::read_file(&md_path)?;
                finalize_start_index(&project_root, &req.idempotency_key, &reservation)?;
                return Ok(AgentOpResult {
                    run_id,
                    state: agent.lifecycle,
                    record_path: rel,
                    absolute_path: md_path,
                    content_hash: content_hash(&markdown),
                    record_revision: 1,
                    project_id: Some(project.project_id),
                    project_root: Some(project_root),
                    op_id: None,
                    git: Some(git),
                    review_state: None,
                    decision_current: None,
                    idempotent_replay: true,
                });
            }
        }
    }
    write_atomic(&md_path, markdown.as_bytes())?;
    write_run_meta_unlocked(&md_path, &meta)?;
    drop(_lock);

    finalize_start_index(&project_root, &req.idempotency_key, &reservation)?;

    if let Some(sk) = session_key.as_deref() {
        let _ = register_session_run(&project_root, sk, run_id, false);
    }

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
        idempotent_replay: reservation.status == StartOpStatus::Complete,
    })
}

fn confirm_provisional_run(
    project_root: &Path,
    project_id: Uuid,
    session_key: &str,
    md_path: PathBuf,
    _meta: RunMeta,
    objective: &str,
    idempotency_key: &str,
) -> Result<AgentOpResult> {
    let side = moraine_sidecar_path(&md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = load_or_migrate_locked(&md_path)?;
    let mut agent = meta
        .agent
        .take()
        .ok_or_else(|| Error::RunRecordStructureInvalid {
            message: "missing agent state".into(),
        })?;

    if !agent.provisional {
        let markdown = Document::read_file(&md_path)?;
        return Ok(AgentOpResult {
            run_id: meta.run.id,
            state: agent.lifecycle,
            record_path: agent.record_path.clone(),
            absolute_path: md_path,
            content_hash: content_hash(&markdown),
            record_revision: agent.record_revision,
            project_id: Some(project_id),
            project_root: Some(project_root.to_path_buf()),
            op_id: None,
            git: agent.starting_git.clone(),
            review_state: None,
            decision_current: None,
            idempotent_replay: true,
        });
    }

    if agent.start_idempotency_key == idempotency_key && agent.objective == objective {
        let markdown = Document::read_file(&md_path)?;
        return Ok(AgentOpResult {
            run_id: meta.run.id,
            state: agent.lifecycle,
            record_path: agent.record_path.clone(),
            absolute_path: md_path,
            content_hash: content_hash(&markdown),
            record_revision: agent.record_revision,
            project_id: Some(project_id),
            project_root: Some(project_root.to_path_buf()),
            op_id: None,
            git: agent.starting_git.clone(),
            review_state: None,
            decision_current: None,
            idempotent_replay: true,
        });
    }

    let payload_hash = hash_payload(&json!({
        "kind": "start",
        "objective": objective,
        "confirm": true,
        "sessionKey": session_key,
    }));

    update_project_meta(project_root, |pm| {
        if let Some(existing) = pm.start_ops.get(idempotency_key) {
            if existing.run_id != meta.run.id || existing.objective != objective {
                return Err(Error::IdempotencyConflict {
                    key: idempotency_key.to_string(),
                    message: "start idempotency key was reused with a different run or objective"
                        .into(),
                });
            }
            return Ok(());
        }
        pm.start_ops.insert(
            idempotency_key.to_string(),
            StartOpIndex {
                run_id: meta.run.id,
                objective: objective.to_string(),
                record_path: agent.record_path.clone(),
                payload_hash: payload_hash.clone(),
                status: StartOpStatus::Complete,
            },
        );
        Ok(())
    })?;

    let human = {
        let markdown = Document::read_file(&md_path)?;
        extract_human_notes(&markdown)?
    };

    agent.objective = objective.to_string();
    agent.provisional = false;
    agent.start_idempotency_key = idempotency_key.to_string();
    agent.session_id = Some(session_key.to_string());
    let session = load_session(project_root, session_key)?;
    agent.capture_coverage =
        derive_capture_coverage(false, session.as_ref(), agent.checkpoints.len());
    agent.bump_revision()?;
    let git = capture_git_context(project_root);
    agent.current_git = Some(git.clone());

    let new_md = render_run_markdown_with_id(meta.run.id, &agent, &human);
    let new_hash = content_hash(&new_md);
    write_atomic(&md_path, new_md.as_bytes())?;
    meta.agent = Some(agent.clone());
    meta.touch();
    write_run_meta_unlocked(&md_path, &meta)?;
    drop(_lock);

    register_session_run(project_root, session_key, meta.run.id, true)?;

    Ok(AgentOpResult {
        run_id: meta.run.id,
        state: agent.lifecycle,
        record_path: agent.record_path.clone(),
        absolute_path: md_path,
        content_hash: new_hash,
        record_revision: agent.record_revision,
        project_id: Some(project_id),
        project_root: Some(project_root.to_path_buf()),
        op_id: None,
        git: Some(git),
        review_state: None,
        decision_current: None,
        idempotent_replay: false,
    })
}

/// Ensure a provisional run exists after the *first* substantive event only.
pub fn provisional_run_ensure(req: ProvisionalRunRequest) -> Result<AgentOpResult> {
    let external = req.session_id.trim();
    if external.is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "session_id is required".into(),
        });
    }

    let observed = session_observe(SessionObserveRequest {
        session_id: external.to_string(),
        integration: "codex".into(),
        project: req.project.clone(),
        source: "provisional_ensure".into(),
        initial_task: req.objective.clone(),
        ended: false,
        confine_existing_project: true,
    })?;

    if let Some(run_id) = observed.active_provisional_run_id {
        if let Ok((md_path, meta)) = find_run_by_id(&observed.project_root, run_id) {
            if let Some(agent) = meta.agent.as_ref() {
                let markdown = Document::read_file(&md_path)?;
                return Ok(AgentOpResult {
                    run_id: meta.run.id,
                    state: agent.lifecycle,
                    record_path: agent.record_path.clone(),
                    absolute_path: md_path,
                    content_hash: content_hash(&markdown),
                    record_revision: agent.record_revision,
                    project_id: Some(observed.project_id),
                    project_root: Some(observed.project_root),
                    op_id: None,
                    git: agent.starting_git.clone(),
                    review_state: None,
                    decision_current: None,
                    idempotent_replay: true,
                });
            }
        }
    }

    if !observed.run_ids.is_empty() {
        if let Some(run_id) = observed.run_ids.last().copied() {
            if let Ok((md_path, meta)) = find_run_by_id(&observed.project_root, run_id) {
                if let Some(agent) = meta.agent.as_ref() {
                    let markdown = Document::read_file(&md_path)?;
                    return Ok(AgentOpResult {
                        run_id: meta.run.id,
                        state: agent.lifecycle,
                        record_path: agent.record_path.clone(),
                        absolute_path: md_path,
                        content_hash: content_hash(&markdown),
                        record_revision: agent.record_revision,
                        project_id: Some(observed.project_id),
                        project_root: Some(observed.project_root),
                        op_id: None,
                        git: agent.starting_git.clone(),
                        review_state: None,
                        decision_current: None,
                        idempotent_replay: true,
                    });
                }
            }
        }
    }

    let objective = require_safe_scalar(
        "objective",
        req.objective
            .as_deref()
            .or(observed.initial_task.as_deref())
            .unwrap_or("Provisional capture (no objective yet)")
            .trim(),
        MAX_SUMMARY_CHARS,
    )?;
    let session_key = observed.session_key.clone();
    let idempotency_key = req
        .idempotency_key
        .unwrap_or_else(|| format!("provisional:{session_key}"));

    let payload_hash = hash_payload(&json!({
        "kind": "provisional_start",
        "sessionKey": session_key,
        "objective": objective,
    }));

    let project_root = observed.project_root.clone();
    let project_id = observed.project_id;

    let reservation = update_project_meta(&project_root, |meta| {
        if let Some(existing) = meta.start_ops.get(&idempotency_key).cloned() {
            return Ok(existing);
        }
        let run_id = Uuid::new_v4();
        let short = short_id(run_id);
        let date = Utc::now().format("%Y-%m-%d");
        let slug = slugify(&objective);
        let mut file_name = format!("{date}-{slug}-{short}.md");
        let runs = runs_dir(&project_root);
        let mut md_path = runs.join(&file_name);
        if md_path.exists() {
            file_name = format!(
                "{date}-{slug}-{short}-{}.md",
                &Uuid::new_v4().to_string()[..8]
            );
            md_path = runs.join(&file_name);
        }
        let rel = path_relative_to(&md_path, &project_root);
        let entry = StartOpIndex {
            run_id,
            objective: objective.clone(),
            record_path: rel,
            payload_hash: payload_hash.clone(),
            status: StartOpStatus::Pending,
        };
        meta.start_ops
            .insert(idempotency_key.clone(), entry.clone());
        Ok(entry)
    })?;

    let md_path = project_root.join(&reservation.record_path);
    let rel = reservation.record_path.clone();
    let run_id = reservation.run_id;

    if reservation.status == StartOpStatus::Complete && md_path.is_file() {
        if let Some(meta) = crate::run_meta::load_run_meta_readonly(&md_path)? {
            if let Some(agent) = meta.agent.as_ref() {
                let markdown = Document::read_file(&md_path)?;
                set_session_provisional_run(&project_root, &session_key, run_id)?;
                return Ok(AgentOpResult {
                    run_id: meta.run.id,
                    state: agent.lifecycle,
                    record_path: agent.record_path.clone(),
                    absolute_path: md_path,
                    content_hash: content_hash(&markdown),
                    record_revision: agent.record_revision,
                    project_id: Some(project_id),
                    project_root: Some(project_root),
                    op_id: None,
                    git: agent.starting_git.clone(),
                    review_state: None,
                    decision_current: None,
                    idempotent_replay: true,
                });
            }
        }
    }

    let git = capture_git_context(&project_root);
    let session = load_session(&project_root, &session_key)?;
    let coverage = derive_capture_coverage(true, session.as_ref(), 0);
    let agent = AgentRunState {
        lifecycle: RunLifecycle::Active,
        record_revision: 1,
        objective: objective.clone(),
        record_path: rel.clone(),
        project_id: Some(project_id),
        start_idempotency_key: idempotency_key.clone(),
        starting_git: Some(git.clone()),
        current_git: Some(git.clone()),
        checkpoints: vec![],
        lifecycle_events: vec![],
        ready_summary: None,
        idempotency: Default::default(),
        incomplete_op: None,
        risks: vec![],
        open_questions: vec![],
        capture_coverage: coverage,
        session_id: Some(session_key.clone()),
        provisional: true,
        evidence: vec![],
        findings: vec![],
        finding_events: vec![],
        append_only_ops: vec![],
    };

    let mut meta = RunMeta::new_run_with_id(run_id);
    meta.schema_version = SCHEMA_VERSION;
    meta.agent = Some(agent.clone());

    let markdown = render_run_markdown_with_id(run_id, &agent, "");
    let hash = content_hash(&markdown);

    let side = moraine_sidecar_path(&md_path);
    let _lock = SidecarLock::acquire(&side)?;
    if md_path.is_file() {
        if let Some(existing) = crate::run_meta::load_run_meta_readonly(&md_path)? {
            if existing.run.id == run_id {
                let markdown = Document::read_file(&md_path)?;
                finalize_start_index(&project_root, &idempotency_key, &reservation)?;
                set_session_provisional_run(&project_root, &session_key, run_id)?;
                return Ok(AgentOpResult {
                    run_id,
                    state: agent.lifecycle,
                    record_path: rel,
                    absolute_path: md_path,
                    content_hash: content_hash(&markdown),
                    record_revision: 1,
                    project_id: Some(project_id),
                    project_root: Some(project_root),
                    op_id: None,
                    git: Some(git),
                    review_state: None,
                    decision_current: None,
                    idempotent_replay: true,
                });
            }
        }
    }
    write_atomic(&md_path, markdown.as_bytes())?;
    write_run_meta_unlocked(&md_path, &meta)?;
    drop(_lock);

    finalize_start_index(&project_root, &idempotency_key, &reservation)?;
    set_session_provisional_run(&project_root, &session_key, run_id)?;

    Ok(AgentOpResult {
        run_id,
        state: RunLifecycle::Active,
        record_path: rel,
        absolute_path: md_path,
        content_hash: hash,
        record_revision: 1,
        project_id: Some(project_id),
        project_root: Some(project_root),
        op_id: None,
        git: Some(git),
        review_state: None,
        decision_current: None,
        idempotent_replay: reservation.status == StartOpStatus::Complete,
    })
}

fn finalize_start_index(project_root: &Path, key: &str, reservation: &StartOpIndex) -> Result<()> {
    update_project_meta(project_root, |pm| {
        if let Some(entry) = pm.start_ops.get_mut(key) {
            if entry.run_id == reservation.run_id {
                entry.status = StartOpStatus::Complete;
            }
        } else {
            let mut e = reservation.clone();
            e.status = StartOpStatus::Complete;
            pm.start_ops.insert(key.to_string(), e);
        }
        Ok(())
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
            ctx.agent.bump_revision()?;
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
    let summary = match summary {
        Some(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(require_safe_scalar("summary", t, MAX_SUMMARY_CHARS)?)
            }
        }
        None => None,
    };
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
            ctx.agent.bump_revision()?;
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
    let reason = match reason {
        Some(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(require_safe_scalar("reason", t, MAX_SUMMARY_CHARS)?)
            }
        }
        None => None,
    };
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
            ctx.agent.bump_revision()?;
            Ok((op_id, Some(git)))
        },
    )
}

pub fn run_show(
    project: Option<&Path>,
    run_id: Uuid,
    opts: RunShowOptions,
) -> Result<RunShowPacket> {
    let project = resolve_existing_project(project)?;
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
        risks: bound_list(&agent.risks, MAX_RECENT_LIST_IN_SHOW),
        open_questions: bound_list(&agent.open_questions, MAX_RECENT_LIST_IN_SHOW),
        annotations: counts,
        review_state: review_state_str(snap.state).into(),
        decision_current: snap.decision_current,
        incomplete_operation: agent.incomplete_op.as_ref().map(|i| IncompleteOpSummary {
            op_id: i.op_id,
            kind: i.kind.clone(),
            phase: i.phase,
            base_content_hash: i.base_content_hash.clone(),
            expected_content_hash: i.expected_content_hash.clone(),
        }),
        // note: full pending_agent is not exposed in show
        markdown: if opts.include_markdown {
            Some(markdown)
        } else {
            None
        },
    })
}

fn bound_list(items: &[String], recent: usize) -> BoundedStringList {
    let total = items.len();
    let recent: Vec<String> = items
        .iter()
        .rev()
        .take(recent)
        .map(|s| truncate(s, 240))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    BoundedStringList { total, recent }
}

struct MutCtx<'a> {
    project_root: &'a Path,
    agent: &'a mut AgentRunState,
}

/// Recover incomplete agent ops before any further mutation of agent state.
///
/// Caller must hold the sidecar lock. When Markdown already matches the pending
/// expected hash, promotes `pending_agent` (so later writers do not wipe concurrent
/// sidecar fields such as findings). When Markdown still matches the base hash,
/// discards the pending mutation. Other hashes require manual recovery.
pub(crate) fn recover_incomplete_agent_op(
    md_path: &Path,
    meta: &mut RunMeta,
    actual_content_hash: &str,
) -> Result<()> {
    let Some(agent) = meta.agent.as_mut() else {
        return Ok(());
    };
    let Some(inc) = agent.incomplete_op.clone() else {
        return Ok(());
    };
    if actual_content_hash == inc.expected_content_hash {
        // Markdown applied; promote pending once.
        let mut pending = (*inc.pending_agent).clone();
        pending.incomplete_op = None;
        pending.record_idempotency(
            inc.idempotency_key.clone(),
            IdempotencyRecord {
                payload_hash: inc.payload_hash.clone(),
                op_id: inc.op_id,
                kind: inc.kind.clone(),
                content_hash: actual_content_hash.to_string(),
                record_revision: pending.record_revision,
                created_at: Utc::now(),
            },
        )?;
        meta.agent = Some(pending);
        write_run_meta_unlocked(md_path, meta)?;
    } else if actual_content_hash == inc.base_content_hash {
        // Markdown never applied; discard pending, keep committed state.
        agent.incomplete_op = None;
        write_run_meta_unlocked(md_path, meta)?;
    } else {
        return Err(Error::OperationRecoveryRequired {
            message: format!(
                "incomplete op {} phase {:?}: document hash matches neither base nor expected",
                inc.op_id, inc.phase
            ),
        });
    }
    Ok(())
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
    // Mutations never auto-create a project (only `run start` may init).
    let project = resolve_existing_project(project)?;
    let project_root = project.project_root.clone();
    let (md_path, _) = find_run_by_id(&project_root, run_id)?;
    let side = moraine_sidecar_path(&md_path);
    let _lock = SidecarLock::acquire(&side)?;

    let mut meta = load_or_migrate_locked(&md_path)?;
    let markdown = Document::read_file(&md_path)?;
    let actual = content_hash(&markdown);

    // Recover incomplete ops before applying a new mutation.
    recover_incomplete_agent_op(&md_path, &mut meta, &actual)?;

    meta = load_or_migrate_locked(&md_path)?;
    let markdown = Document::read_file(&md_path)?;
    let actual = content_hash(&markdown);

    let agent = meta
        .agent
        .as_mut()
        .ok_or_else(|| Error::other("run missing agent state"))?;

    // Lifetime idempotency (no silent eviction).
    if let Some(prev) = agent.find_idempotency(idempotency_key).cloned() {
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

    // Preflight capacity for a *new* key before any file mutation.
    if !agent.has_idempotency_capacity_for(idempotency_key) {
        return Err(Error::IdempotencyIndexFull {
            max: crate::agent_protocol::types::MAX_IDEMPOTENCY_INDEX,
        });
    }

    let human = extract_human_notes(&markdown)?;
    // Apply mutation only to a pending clone; committed agent stays intact until MD succeeds.
    let mut pending = agent.clone();
    pending.incomplete_op = None;
    let mut ctx = MutCtx {
        project_root: &project_root,
        agent: &mut pending,
    };
    let (op_id, git) = apply(&mut ctx)?;

    let new_md = render_run_markdown_with_id(run_id, &pending, &human);
    let new_hash = content_hash(&new_md);

    // Phase 1: write intent only — committed agent fields unchanged except incomplete_op.
    let mut committed = agent.clone();
    committed.incomplete_op = Some(Box::new(IncompleteOp {
        op_id,
        idempotency_key: idempotency_key.to_string(),
        kind: kind.to_string(),
        payload_hash: payload_hash.to_string(),
        base_content_hash: actual.clone(),
        expected_content_hash: new_hash.clone(),
        phase: IncompletePhase::Begun,
        created_at: Utc::now(),
        pending_agent: Box::new(pending.clone()),
    }));
    meta.agent = Some(committed);
    write_run_meta_unlocked(&md_path, &meta)?;

    // Phase 2: Markdown. If this fails, committed agent state (without the mutation) remains
    // with incomplete_op; recovery on base hash discards the pending mutation.
    write_atomic(&md_path, new_md.as_bytes())?;

    // Phase 3: promote pending → committed.
    pending.incomplete_op = None;
    pending.record_idempotency(
        idempotency_key.to_string(),
        IdempotencyRecord {
            payload_hash: payload_hash.to_string(),
            op_id,
            kind: kind.to_string(),
            content_hash: new_hash.clone(),
            record_revision: pending.record_revision,
            created_at: Utc::now(),
        },
    )?;
    meta.agent = Some(pending.clone());
    meta.touch();
    write_run_meta_unlocked(&md_path, &meta)?;

    let snap = review_snapshot(&meta, &new_md);

    Ok(AgentOpResult {
        run_id,
        state: pending.lifecycle,
        record_path: pending.record_path.clone(),
        absolute_path: md_path,
        content_hash: new_hash,
        record_revision: pending.record_revision,
        project_id: pending.project_id.or(Some(project.project_id)),
        project_root: Some(project_root),
        op_id: Some(op_id),
        git,
        review_state: Some(review_state_str(snap.state).into()),
        decision_current: Some(snap.decision_current),
        idempotent_replay: false,
    })
}

fn review_state_str(s: crate::run_meta::ReviewStateKind) -> &'static str {
    match s {
        crate::run_meta::ReviewStateKind::Unreviewed => "unreviewed",
        crate::run_meta::ReviewStateKind::Approved => "approved",
        crate::run_meta::ReviewStateKind::ChangesRequested => "changes_requested",
        crate::run_meta::ReviewStateKind::Rejected => "rejected",
        crate::run_meta::ReviewStateKind::Stale => "stale",
    }
}

/// Reject CR/LF and other control characters that can inject Markdown structure.
pub fn require_safe_scalar(field: &str, value: &str, max: usize) -> Result<String> {
    if value.len() > max {
        return Err(Error::InvalidCheckpoint {
            message: format!("{field} exceeds {max} characters"),
        });
    }
    for ch in value.chars() {
        if ch == '\n' || ch == '\r' || ch == '\0' || (ch.is_control() && ch != '\t') {
            return Err(Error::InvalidCheckpoint {
                message: format!(
                    "{field} must not contain newlines or control characters (Markdown-structure safety)"
                ),
            });
        }
    }
    // Block heading-injection tokens that could brick the record if embedded oddly.
    if value.contains("## Human notes") || value.contains("\n#") {
        return Err(Error::InvalidCheckpoint {
            message: format!("{field} must not contain Markdown heading markers"),
        });
    }
    Ok(value.to_string())
}

fn validate_checkpoint(mut input: CheckpointInput) -> Result<CheckpointInput> {
    input.summary = require_safe_scalar("summary", input.summary.trim(), MAX_SUMMARY_CHARS)?;
    if input.summary.is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "summary is required".into(),
        });
    }

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

    let mut actions = Vec::with_capacity(input.actions.len());
    for a in input.actions {
        actions.push(require_safe_scalar("action", a.trim(), MAX_FIELD_CHARS)?);
    }
    input.actions = actions;

    for r in &mut input.rationales {
        r.choice = require_safe_scalar("rationale.choice", r.choice.trim(), MAX_FIELD_CHARS)?;
        r.reason = require_safe_scalar("rationale.reason", r.reason.trim(), MAX_FIELD_CHARS)?;
        if r.choice.is_empty() || r.reason.is_empty() {
            return Err(Error::InvalidCheckpoint {
                message: "rationale choice and reason are required".into(),
            });
        }
    }

    let mut risks = Vec::with_capacity(input.risks.len());
    for r in input.risks {
        risks.push(require_safe_scalar("risk", r.trim(), MAX_FIELD_CHARS)?);
    }
    input.risks = risks;

    let mut oq = Vec::with_capacity(input.open_questions.len());
    for q in input.open_questions {
        oq.push(require_safe_scalar(
            "openQuestion",
            q.trim(),
            MAX_FIELD_CHARS,
        )?);
    }
    input.open_questions = oq;

    for e in &mut input.evidence {
        e.label = require_safe_scalar("evidence.label", e.label.trim(), MAX_FIELD_CHARS)?;
        if e.label.is_empty() {
            return Err(Error::InvalidCheckpoint {
                message: "evidence label is required".into(),
            });
        }
        if let Some(cmd) = e.command.take() {
            e.command = Some(require_safe_scalar(
                "evidence.command",
                cmd.trim(),
                MAX_FIELD_CHARS,
            )?);
        }
        if let Some(p) = e.path.take() {
            e.path = Some(require_safe_scalar(
                "evidence.path",
                p.trim(),
                MAX_FIELD_CHARS,
            )?);
        }
        if let Some(u) = e.url.take() {
            e.url = Some(require_safe_scalar(
                "evidence.url",
                u.trim(),
                MAX_FIELD_CHARS,
            )?);
        }
        // Agent-supplied evidence can never claim Moraine capture.
        if e.provenance == EvidenceProvenance::MoraineCaptured {
            return Err(Error::InvalidCheckpoint {
                message: "provenance moraine_captured is not allowed on agent checkpoint evidence"
                    .into(),
            });
        }
        e.provenance = EvidenceProvenance::AgentReported;
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

/// Test helper: inject incomplete intent without writing Markdown (fault injection).
#[cfg(test)]
pub fn test_begin_incomplete_without_markdown(
    md_path: &Path,
    pending: AgentRunState,
    incomplete: IncompleteOp,
) -> Result<()> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = load_or_migrate_locked(md_path)?;
    let mut agent = meta.agent.take().ok_or_else(|| Error::other("no agent"))?;
    // Keep committed state; store incomplete with pending.
    let mut inc = incomplete;
    inc.pending_agent = Box::new(pending);
    agent.incomplete_op = Some(Box::new(inc));
    meta.agent = Some(agent);
    write_run_meta_unlocked(md_path, &meta)
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
            session_id: None,
        })
        .unwrap();
        assert_eq!(start.state, RunLifecycle::Active);

        let md = Document::read_file(&start.absolute_path).unwrap();
        assert!(md.contains("## Human notes"));
        assert!(md.contains("Managed regions"));

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
        assert_eq!(show.risks.total, 1);
        assert_eq!(show.risks.recent.len(), 1);

        let _ = record_decision(
            &ready.absolute_path,
            DecisionKind::Approved,
            "tester",
            None,
            &ready.content_hash,
        )
        .unwrap();

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
    }

    #[test]
    fn failed_markdown_write_does_not_commit_checkpoint() {
        let dir = tempdir().unwrap();
        let start = run_start(RunStartRequest {
            objective: "Recovery".into(),
            idempotency_key: "s".into(),
            project: Some(dir.path().to_path_buf()),
            session_id: None,
        })
        .unwrap();

        // Simulate: incomplete intent written, Markdown still at base.
        let meta = load_or_migrate_locked(&start.absolute_path).unwrap();
        let agent = meta.agent.as_ref().unwrap().clone();
        let mut pending = agent.clone();
        pending.checkpoints.push(CheckpointRecord {
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
        let pending_md = render_run_markdown_with_id(start.run_id, &pending, "");
        let expected = content_hash(&pending_md);
        test_begin_incomplete_without_markdown(
            &start.absolute_path,
            pending,
            IncompleteOp {
                op_id: Uuid::new_v4(),
                idempotency_key: "ghost".into(),
                kind: "checkpoint".into(),
                payload_hash: "x".into(),
                base_content_hash: start.content_hash.clone(),
                expected_content_hash: expected,
                phase: IncompletePhase::Begun,
                created_at: Utc::now(),
                pending_agent: Box::new(agent.clone()),
            },
        )
        .unwrap();

        // Next mutation with base hash recovers by discarding pending.
        let cp = run_checkpoint(
            Some(dir.path()),
            start.run_id,
            &start.content_hash,
            "real-cp",
            CheckpointInput {
                summary: "real".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
            },
        )
        .unwrap();
        assert_eq!(cp.record_revision, 2);
        let show = run_show(Some(dir.path()), start.run_id, RunShowOptions::default()).unwrap();
        assert_eq!(show.checkpoint_count, 1);
        let md = Document::read_file(&start.absolute_path).unwrap();
        assert!(!md.contains("should not appear"));
        assert!(md.contains("real"));
    }

    #[test]
    fn rejects_structure_injection_and_moraine_captured() {
        let err = require_safe_scalar("summary", "hi\n## Human notes", 100).unwrap_err();
        assert!(matches!(err, Error::InvalidCheckpoint { .. }));

        let err = validate_checkpoint(CheckpointInput {
            summary: "ok".into(),
            actions: vec![],
            rationales: vec![],
            evidence: vec![EvidenceItem {
                kind: crate::agent_protocol::types::EvidenceKind::Note,
                label: "x".into(),
                command: None,
                exit_code: None,
                path: None,
                url: None,
                provenance: EvidenceProvenance::MoraineCaptured,
            }],
            risks: vec![],
            open_questions: vec![],
        })
        .unwrap_err();
        assert!(matches!(err, Error::InvalidCheckpoint { .. }));
    }

    #[test]
    fn start_idempotency_conflict() {
        let dir = tempdir().unwrap();
        let _ = run_start(RunStartRequest {
            objective: "A".into(),
            idempotency_key: "k".into(),
            project: Some(dir.path().to_path_buf()),
            session_id: None,
        })
        .unwrap();
        let err = run_start(RunStartRequest {
            objective: "B".into(),
            idempotency_key: "k".into(),
            project: Some(dir.path().to_path_buf()),
            session_id: None,
        })
        .unwrap_err();
        assert!(matches!(err, Error::IdempotencyConflict { .. }));
    }

    #[test]
    fn concurrent_starts_same_key_one_run() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let barrier = Arc::new(Barrier::new(2));
        let b1 = barrier.clone();
        let r1 = root.clone();
        let t1 = thread::spawn(move || {
            b1.wait();
            run_start(RunStartRequest {
                objective: "Concurrent start".into(),
                idempotency_key: "same".into(),
                project: Some(r1),
                session_id: None,
            })
        });
        let b2 = barrier;
        let r2 = root.clone();
        let t2 = thread::spawn(move || {
            b2.wait();
            run_start(RunStartRequest {
                objective: "Concurrent start".into(),
                idempotency_key: "same".into(),
                project: Some(r2),
                session_id: None,
            })
        });
        let a = t1.join().unwrap().unwrap();
        let b = t2.join().unwrap().unwrap();
        assert_eq!(a.run_id, b.run_id);
        // Only one run file for this key.
        let runs = runs_dir(&root);
        let count = std::fs::read_dir(&runs)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn concurrent_checkpoints_one_wins() {
        let dir = tempdir().unwrap();
        let start = run_start(RunStartRequest {
            objective: "Concurrent".into(),
            idempotency_key: "s".into(),
            project: Some(dir.path().to_path_buf()),
            session_id: None,
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
        assert_eq!(conflict, 1);

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
            session_id: None,
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
                    risks: vec![format!("risk-{i}-{}", "x".repeat(50))],
                    open_questions: vec![format!("q-{i}")],
                },
            )
            .unwrap();
        }
        let show = run_show(Some(dir.path()), cur.run_id, RunShowOptions::default()).unwrap();
        assert_eq!(show.checkpoint_count, 30);
        assert!(show.recent_checkpoints.len() <= MAX_RECENT_CHECKPOINTS_IN_SHOW);
        assert_eq!(show.risks.total, 30);
        assert!(show.risks.recent.len() <= MAX_RECENT_LIST_IN_SHOW);
        let packed = serde_json::to_vec(&show).unwrap();
        assert!(
            packed.len() < 4096,
            "default show should stay compact, got {}",
            packed.len()
        );
    }

    #[test]
    fn show_does_not_create_project() {
        let dir = tempdir().unwrap();
        let err =
            run_show(Some(dir.path()), Uuid::new_v4(), RunShowOptions::default()).unwrap_err();
        assert!(matches!(err, Error::ProjectNotFound { .. }));
        assert!(!dir.path().join(".moraine").exists());
    }

    #[test]
    fn checkpoint_ready_resume_do_not_create_project() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let id = Uuid::new_v4();
        let err = run_checkpoint(
            Some(root),
            id,
            "abc",
            "k",
            CheckpointInput {
                summary: "x".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::ProjectNotFound { .. }));
        let err = run_ready(Some(root), id, "abc", "k", None).unwrap_err();
        assert!(matches!(err, Error::ProjectNotFound { .. }));
        let err = run_resume(Some(root), id, "abc", "k", None).unwrap_err();
        assert!(matches!(err, Error::ProjectNotFound { .. }));
        assert!(!root.join(".moraine").exists());
    }

    #[test]
    fn idempotency_capacity_preflight_leaves_files_unchanged() {
        use crate::agent_protocol::types::{IdempotencyRecord, MAX_IDEMPOTENCY_INDEX};

        let dir = tempdir().unwrap();
        let start = run_start(RunStartRequest {
            objective: "Capacity".into(),
            idempotency_key: "s".into(),
            project: Some(dir.path().to_path_buf()),
            session_id: None,
        })
        .unwrap();

        let side = moraine_sidecar_path(&start.absolute_path);
        let _lock = SidecarLock::acquire(&side).unwrap();
        let mut meta = load_or_migrate_locked(&start.absolute_path).unwrap();
        let agent = meta.agent.as_mut().unwrap();
        let now = Utc::now();
        let replay_payload = CheckpointInput {
            summary: "replay".into(),
            actions: vec![],
            rationales: vec![],
            evidence: vec![],
            risks: vec![],
            open_questions: vec![],
        };
        let replay_hash = hash_payload(&serde_json::to_value(&replay_payload).unwrap());
        agent.idempotency.clear();
        for i in 0..(MAX_IDEMPOTENCY_INDEX - 1) {
            agent.idempotency.insert(
                format!("fill-{i}"),
                IdempotencyRecord {
                    payload_hash: format!("p{i}"),
                    op_id: Uuid::new_v4(),
                    kind: "checkpoint".into(),
                    content_hash: start.content_hash.clone(),
                    record_revision: 1,
                    created_at: now,
                },
            );
        }
        agent.idempotency.insert(
            "replay-me".into(),
            IdempotencyRecord {
                payload_hash: replay_hash,
                op_id: Uuid::new_v4(),
                kind: "checkpoint".into(),
                content_hash: start.content_hash.clone(),
                record_revision: 1,
                created_at: now,
            },
        );
        assert_eq!(agent.idempotency.len(), MAX_IDEMPOTENCY_INDEX);
        let rev_before = agent.record_revision;
        write_run_meta_unlocked(&start.absolute_path, &meta).unwrap();
        drop(_lock);

        let md_before = Document::read_file(&start.absolute_path).unwrap();
        let side_before =
            std::fs::read_to_string(moraine_sidecar_path(&start.absolute_path)).unwrap();

        let err = run_checkpoint(
            Some(dir.path()),
            start.run_id,
            &start.content_hash,
            "brand-new-key",
            CheckpointInput {
                summary: "overflow".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec![],
                open_questions: vec![],
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::IdempotencyIndexFull { .. }), "{err:?}");

        assert_eq!(
            Document::read_file(&start.absolute_path).unwrap(),
            md_before
        );
        assert_eq!(
            std::fs::read_to_string(moraine_sidecar_path(&start.absolute_path)).unwrap(),
            side_before
        );

        let meta2 = load_or_migrate_locked(&start.absolute_path).unwrap();
        let agent2 = meta2.agent.as_ref().unwrap();
        assert!(agent2.incomplete_op.is_none());
        assert_eq!(agent2.record_revision, rev_before);

        let replay = run_checkpoint(
            Some(dir.path()),
            start.run_id,
            &start.content_hash,
            "replay-me",
            replay_payload,
        )
        .unwrap();
        assert!(replay.idempotent_replay);
        assert_eq!(replay.content_hash, start.content_hash);
    }

    #[test]
    fn provisional_ensure_then_run_start_confirms_same_run() {
        let dir = tempdir().unwrap();
        let project = init_project(Some(dir.path())).unwrap();
        let external = "sess-dogfood-1";

        // First substantive prompt path: observe then ensure.
        let obs = crate::agent_protocol::session::session_observe(
            crate::agent_protocol::session::SessionObserveRequest {
                session_id: external.into(),
                integration: "codex".into(),
                project: Some(project.project_root.clone()),
                source: "user_prompt".into(),
                initial_task: Some("Fix the spool".into()),
                ended: false,
                confine_existing_project: true,
            },
        )
        .unwrap();
        assert!(obs.should_ensure_provisional);

        let first = provisional_run_ensure(ProvisionalRunRequest {
            session_id: external.into(),
            project: Some(project.project_root.clone()),
            objective: Some("Fix the spool".into()),
            idempotency_key: None,
        })
        .unwrap();
        assert!(!first.idempotent_replay);

        let meta = load_or_migrate_locked(&first.absolute_path).unwrap();
        let agent = meta.agent.as_ref().unwrap();
        assert!(agent.provisional);
        assert_eq!(
            agent.capture_coverage,
            crate::agent_protocol::CaptureCoverage::MechanicalOnly
        );
        assert_eq!(agent.session_id.as_deref(), Some(obs.session_key.as_str()));

        let second = provisional_run_ensure(ProvisionalRunRequest {
            session_id: external.into(),
            project: Some(project.project_root.clone()),
            objective: Some("Fix the spool".into()),
            idempotency_key: None,
        })
        .unwrap();
        assert!(second.idempotent_replay);
        assert_eq!(second.run_id, first.run_id);

        let confirmed = run_start(RunStartRequest {
            objective: "Fix the spool and add tests".into(),
            idempotency_key: "mcp-start-1".into(),
            project: Some(project.project_root.clone()),
            session_id: Some(external.into()),
        })
        .unwrap();
        assert_eq!(confirmed.run_id, first.run_id);
        assert!(!confirmed.idempotent_replay);

        let meta2 = load_or_migrate_locked(&confirmed.absolute_path).unwrap();
        let agent2 = meta2.agent.as_ref().unwrap();
        assert!(!agent2.provisional);
        assert_eq!(
            agent2.capture_coverage,
            crate::agent_protocol::CaptureCoverage::Full
        );
        assert_eq!(agent2.objective, "Fix the spool and add tests");

        // Second explicit start in same session with a *new* key creates a new run.
        let second_run = run_start(RunStartRequest {
            objective: "Unrelated defect Y".into(),
            idempotency_key: "mcp-start-2".into(),
            project: Some(project.project_root.clone()),
            session_id: Some(external.into()),
        })
        .unwrap();
        assert_ne!(second_run.run_id, first.run_id);

        let replay = run_start(RunStartRequest {
            objective: "Fix the spool and add tests".into(),
            idempotency_key: "mcp-start-1".into(),
            project: Some(project.project_root.clone()),
            session_id: Some(external.into()),
        })
        .unwrap();
        assert!(replay.idempotent_replay);
        assert_eq!(replay.run_id, first.run_id);
    }
}
