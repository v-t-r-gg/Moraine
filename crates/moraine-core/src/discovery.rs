//! Non-mutating project/run discovery and ledger timeline read models (M5).
//!
//! These helpers never promote schemas, rewrite Markdown, or alter sidecars.
//! The service index is a rebuildable cache only; run bundles remain canonical.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent_protocol::{
    current_checkpoint_claim, is_redacted, project_meta_path, resolve_existing_project, runs_dir,
    AgentRunState, FindingState, LedgerRelationship, MORAINE_DIR, OP_ENTRY_REDACT,
    OP_ENTRY_SUPERSEDE, OP_HUMAN_OBSERVATION_ADD, OP_RUN_AMEND,
};
use crate::error::{Error, Result};
use crate::run_meta::{content_hash, load_run_meta_readonly, moraine_sidecar_path, RunMeta};

/// Integrity of a discovered run relative to disk.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunIntegrity {
    Current,
    MissingMarkdown,
    MissingSidecar,
    MalformedSidecar,
    UnsupportedSchema,
    RecoveryRequired,
    Unavailable,
}

impl RunIntegrity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::MissingMarkdown => "missing_markdown",
            Self::MissingSidecar => "missing_sidecar",
            Self::MalformedSidecar => "malformed_sidecar",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::RecoveryRequired => "recovery_required",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunCounts {
    pub active: usize,
    pub ready: usize,
    pub recent: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project_id: Uuid,
    pub name: String,
    pub root_path: String,
    pub available: bool,
    pub run_counts: ProjectRunCounts,
    pub open_finding_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub run_id: Uuid,
    pub project_id: Uuid,
    pub objective: String,
    pub lifecycle: String,
    pub provisional: bool,
    pub capture_coverage: String,
    pub record_path: String,
    pub absolute_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub checkpoint_count: usize,
    pub evidence_count: usize,
    pub open_finding_count: usize,
    pub risk_count: usize,
    pub open_question_count: usize,
    pub append_only_op_count: usize,
    pub integrity: String,
    pub recovery_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Content hash of Markdown when readable (for nonmutation tests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub id: String,
    pub timestamp: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    /// Sort key: (timestamp, kind_rank, id) for deterministic ties.
    #[serde(skip)]
    pub sort_key: (String, u8, String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub summary: RunSummary,
    pub timeline: Vec<TimelineEntry>,
    pub is_protocol_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

/// Infer a display name from the project root path.
pub fn project_display_name(root: &Path) -> String {
    root.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| root.display().to_string())
}

/// Canonicalize when possible; never creates directories.
pub fn canonicalize_existing(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Summarize one initialized Moraine project (read-only).
pub fn summarize_project(root: &Path) -> Result<ProjectSummary> {
    let root = canonicalize_existing(root);
    let resolved = match resolve_existing_project(Some(&root)) {
        Ok(r) => r,
        Err(e) => {
            return Ok(ProjectSummary {
                project_id: Uuid::nil(),
                name: project_display_name(&root),
                root_path: root.display().to_string(),
                available: false,
                run_counts: ProjectRunCounts {
                    active: 0,
                    ready: 0,
                    recent: 0,
                },
                open_finding_count: 0,
                last_activity_at: None,
                warning: Some(e.to_string()),
            });
        }
    };
    let available = root.is_dir() && project_meta_path(&root).is_file();
    let runs = list_run_summaries(&root, resolved.project_id);
    let mut active = 0usize;
    let mut ready = 0usize;
    let mut open_findings = 0usize;
    let mut last: Option<DateTime<Utc>> = None;
    for r in &runs {
        if r.lifecycle == "active" && r.integrity == RunIntegrity::Current.as_str() {
            active += 1;
        }
        if r.lifecycle == "ready_for_review" {
            ready += 1;
        }
        open_findings += r.open_finding_count;
        if let Some(u) = r
            .updated_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        {
            let u = u.with_timezone(&Utc);
            last = Some(last.map(|p| p.max(u)).unwrap_or(u));
        }
    }
    Ok(ProjectSummary {
        project_id: resolved.project_id,
        name: project_display_name(&root),
        root_path: root.display().to_string(),
        available,
        run_counts: ProjectRunCounts {
            active,
            ready,
            recent: runs.len(),
        },
        open_finding_count: open_findings,
        last_activity_at: last.map(|t| t.to_rfc3339()),
        warning: if available {
            None
        } else {
            Some("project path unavailable".into())
        },
    })
}

/// List run summaries under a project (read-only, never mutates).
pub fn list_run_summaries(project_root: &Path, project_id: Uuid) -> Vec<RunSummary> {
    let runs = runs_dir(project_root);
    if !runs.is_dir() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(&runs)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    entries.sort();
    for md in entries {
        out.push(summarize_run_path(&md, project_id));
    }
    // Deterministic: lifecycle then updated_at desc then run id
    out.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.run_id.cmp(&b.run_id))
    });
    out
}

/// Summarize a single run Markdown path without mutating disk.
pub fn summarize_run_path(md_path: &Path, project_id: Uuid) -> RunSummary {
    let absolute = canonicalize_existing(md_path);
    let record_path = absolute
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!(".moraine/runs/{n}"))
        .unwrap_or_else(|| absolute.display().to_string());

    if !absolute.is_file() {
        return RunSummary {
            run_id: Uuid::nil(),
            project_id,
            objective: String::new(),
            lifecycle: "unknown".into(),
            provisional: false,
            capture_coverage: "unknown".into(),
            record_path,
            absolute_path: absolute.display().to_string(),
            started_at: None,
            updated_at: None,
            checkpoint_count: 0,
            evidence_count: 0,
            open_finding_count: 0,
            risk_count: 0,
            open_question_count: 0,
            append_only_op_count: 0,
            integrity: RunIntegrity::MissingMarkdown.as_str().into(),
            recovery_required: false,
            error: Some("markdown missing".into()),
            content_hash: None,
        };
    }

    let markdown = match fs::read_to_string(&absolute) {
        Ok(s) => s,
        Err(e) => {
            return RunSummary {
                run_id: Uuid::nil(),
                project_id,
                objective: String::new(),
                lifecycle: "unknown".into(),
                provisional: false,
                capture_coverage: "unknown".into(),
                record_path,
                absolute_path: absolute.display().to_string(),
                started_at: None,
                updated_at: None,
                checkpoint_count: 0,
                evidence_count: 0,
                open_finding_count: 0,
                risk_count: 0,
                open_question_count: 0,
                append_only_op_count: 0,
                integrity: RunIntegrity::Unavailable.as_str().into(),
                recovery_required: false,
                error: Some(e.to_string()),
                content_hash: None,
            };
        }
    };
    let hash = content_hash(&markdown);
    let side = moraine_sidecar_path(&absolute);
    if !side.is_file() {
        return RunSummary {
            run_id: Uuid::nil(),
            project_id,
            objective: String::new(),
            lifecycle: "unknown".into(),
            provisional: false,
            capture_coverage: "unknown".into(),
            record_path,
            absolute_path: absolute.display().to_string(),
            started_at: None,
            updated_at: None,
            checkpoint_count: 0,
            evidence_count: 0,
            open_finding_count: 0,
            risk_count: 0,
            open_question_count: 0,
            append_only_op_count: 0,
            integrity: RunIntegrity::MissingSidecar.as_str().into(),
            recovery_required: false,
            error: Some("sidecar missing".into()),
            content_hash: Some(hash),
        };
    }

    let meta = match load_run_meta_readonly(&absolute) {
        Ok(Some(m)) => m,
        Ok(None) => {
            return RunSummary {
                run_id: Uuid::nil(),
                project_id,
                objective: String::new(),
                lifecycle: "unknown".into(),
                provisional: false,
                capture_coverage: "unknown".into(),
                record_path,
                absolute_path: absolute.display().to_string(),
                started_at: None,
                updated_at: None,
                checkpoint_count: 0,
                evidence_count: 0,
                open_finding_count: 0,
                risk_count: 0,
                open_question_count: 0,
                append_only_op_count: 0,
                integrity: RunIntegrity::MissingSidecar.as_str().into(),
                recovery_required: false,
                error: Some("sidecar not loaded".into()),
                content_hash: Some(hash),
            };
        }
        Err(Error::UnsupportedSchemaVersion { version, max }) => {
            return RunSummary {
                run_id: Uuid::nil(),
                project_id,
                objective: String::new(),
                lifecycle: "unknown".into(),
                provisional: false,
                capture_coverage: "unknown".into(),
                record_path,
                absolute_path: absolute.display().to_string(),
                started_at: None,
                updated_at: None,
                checkpoint_count: 0,
                evidence_count: 0,
                open_finding_count: 0,
                risk_count: 0,
                open_question_count: 0,
                append_only_op_count: 0,
                integrity: RunIntegrity::UnsupportedSchema.as_str().into(),
                recovery_required: false,
                error: Some(format!("schema {version} > max {max}")),
                content_hash: Some(hash),
            };
        }
        Err(e) => {
            return RunSummary {
                run_id: Uuid::nil(),
                project_id,
                objective: String::new(),
                lifecycle: "unknown".into(),
                provisional: false,
                capture_coverage: "unknown".into(),
                record_path,
                absolute_path: absolute.display().to_string(),
                started_at: None,
                updated_at: None,
                checkpoint_count: 0,
                evidence_count: 0,
                open_finding_count: 0,
                risk_count: 0,
                open_question_count: 0,
                append_only_op_count: 0,
                integrity: RunIntegrity::MalformedSidecar.as_str().into(),
                recovery_required: false,
                error: Some(e.to_string()),
                content_hash: Some(hash),
            };
        }
    };

    let agent = meta.agent.as_ref();
    let recovery = agent.and_then(|a| a.incomplete_op.as_ref()).is_some();
    let integrity = if recovery {
        RunIntegrity::RecoveryRequired
    } else {
        RunIntegrity::Current
    };

    RunSummary {
        run_id: meta.run.id,
        project_id: agent.and_then(|a| a.project_id).unwrap_or(project_id),
        objective: agent.map(|a| a.objective.clone()).unwrap_or_default(),
        lifecycle: agent
            .map(|a| a.lifecycle.as_str().to_string())
            .unwrap_or_else(|| "unknown".into()),
        provisional: agent.map(|a| a.provisional).unwrap_or(false),
        capture_coverage: agent
            .map(|a| a.capture_coverage.as_str().to_string())
            .unwrap_or_else(|| "unknown".into()),
        record_path: agent.map(|a| a.record_path.clone()).unwrap_or(record_path),
        absolute_path: absolute.display().to_string(),
        started_at: Some(meta.run.created_at.to_rfc3339()),
        updated_at: Some(meta.run.updated_at.to_rfc3339()),
        checkpoint_count: agent.map(|a| a.checkpoints.len()).unwrap_or(0),
        evidence_count: agent.map(|a| a.evidence.len()).unwrap_or(0),
        open_finding_count: agent
            .map(|a| {
                a.findings
                    .iter()
                    .filter(|f| f.state == FindingState::Open)
                    .count()
            })
            .unwrap_or(0),
        risk_count: agent.map(|a| a.risks.len()).unwrap_or(0),
        open_question_count: agent.map(|a| a.open_questions.len()).unwrap_or(0),
        append_only_op_count: agent.map(|a| a.append_only_ops.len()).unwrap_or(0),
        integrity: integrity.as_str().into(),
        recovery_required: recovery,
        error: None,
        content_hash: Some(hash),
    }
}

/// Full run detail + timeline (read-only).
pub fn load_run_detail(md_path: &Path, project_id: Uuid) -> RunDetail {
    let summary = summarize_run_path(md_path, project_id);
    let meta = load_run_meta_readonly(md_path).ok().flatten();
    let agent = meta.as_ref().and_then(|m| m.agent.as_ref());
    let is_protocol = agent.is_some();
    let timeline = agent
        .map(|a| build_timeline(meta.as_ref().unwrap(), a))
        .unwrap_or_default();
    RunDetail {
        summary,
        timeline,
        is_protocol_run: is_protocol,
        objective: agent.map(|a| a.objective.clone()),
        risks: agent.map(|a| a.risks.clone()).unwrap_or_default(),
        open_questions: agent.map(|a| a.open_questions.clone()).unwrap_or_default(),
    }
}

fn kind_rank(kind: &str) -> u8 {
    match kind {
        "run_start" => 0,
        "provisional" => 1,
        "checkpoint" => 2,
        "evidence" => 3,
        "lifecycle" => 4,
        "finding_created" => 5,
        "finding_responded" => 6,
        "finding_state" => 7,
        "observation" => 8,
        "amendment" => 9,
        "supersession" => 10,
        "redaction" => 11,
        _ => 50,
    }
}

/// Build chronological timeline entries from structured agent state.
pub fn build_timeline(meta: &RunMeta, agent: &AgentRunState) -> Vec<TimelineEntry> {
    let mut entries: Vec<TimelineEntry> = Vec::new();

    let start_ts = meta.run.created_at.to_rfc3339();
    entries.push(TimelineEntry {
        id: format!("start:{}", meta.run.id),
        timestamp: start_ts.clone(),
        kind: if agent.provisional {
            "provisional".into()
        } else {
            "run_start".into()
        },
        actor_category: Some("agent".into()),
        target_id: Some(meta.run.id.to_string()),
        summary: if agent.provisional {
            "Provisional run created".into()
        } else {
            format!("Run started: {}", truncate(&agent.objective, 120))
        },
        detail: Some(agent.objective.clone()),
        provenance: None,
        sort_key: (start_ts, kind_rank("run_start"), meta.run.id.to_string()),
    });

    for cp in &agent.checkpoints {
        let ts = cp.created_at.to_rfc3339();
        let current = current_checkpoint_claim(agent, cp.op_id);
        let redacted = is_redacted(agent, cp.op_id);
        let summary = if redacted {
            format!("Checkpoint (redacted): {}", truncate(&current, 100))
        } else if current != cp.summary {
            format!(
                "Checkpoint: {} → {}",
                truncate(&cp.summary, 60),
                truncate(&current, 60)
            )
        } else {
            format!("Checkpoint: {}", truncate(&cp.summary, 120))
        };
        let mut detail = format!("Original claim:\n{}\n", cp.summary);
        for op in &agent.append_only_ops {
            if op.target_id != Some(cp.op_id) {
                continue;
            }
            if op.target_kind.as_deref() != Some("checkpoint") {
                continue;
            }
            match op.relationship {
                LedgerRelationship::Amended => {
                    detail.push_str(&format!(
                        "\nAmendment ({reason}):\nPrior: {prior}\nNew: {new}\n",
                        reason = op.reason,
                        prior = op.previous_content.as_deref().unwrap_or("—"),
                        new = op.new_content.as_deref().unwrap_or("")
                    ));
                }
                LedgerRelationship::Superseded => {
                    detail.push_str(&format!(
                        "\nSupersession: {}\n{}\n",
                        op.reason,
                        op.new_content.as_deref().unwrap_or("")
                    ));
                }
                LedgerRelationship::Redacted => {
                    detail.push_str(&format!(
                        "\nRedaction (explicit): {}\nPrior content retained in ledger.\n",
                        op.reason
                    ));
                }
                LedgerRelationship::Observation => {}
            }
        }
        if current != cp.summary {
            detail.push_str(&format!("\nCurrent statement:\n{current}\n"));
        }
        entries.push(TimelineEntry {
            id: format!("checkpoint:{}", cp.op_id),
            timestamp: ts.clone(),
            kind: "checkpoint".into(),
            actor_category: Some("agent".into()),
            target_id: Some(cp.op_id.to_string()),
            summary,
            detail: Some(detail),
            provenance: None,
            sort_key: (ts, kind_rank("checkpoint"), cp.op_id.to_string()),
        });
    }

    for ev in &agent.lifecycle_events {
        let ts = ev.created_at.to_rfc3339();
        entries.push(TimelineEntry {
            id: format!("lifecycle:{}", ev.op_id),
            timestamp: ts.clone(),
            kind: "lifecycle".into(),
            actor_category: Some("agent".into()),
            target_id: Some(ev.op_id.to_string()),
            summary: format!("Lifecycle: {}", ev.kind),
            detail: ev.note.clone(),
            provenance: None,
            sort_key: (ts, kind_rank("lifecycle"), ev.op_id.to_string()),
        });
    }

    for e in &agent.evidence {
        let ts = e.occurred_at.to_rfc3339();
        entries.push(TimelineEntry {
            id: format!("evidence:{}", e.evidence_id),
            timestamp: ts.clone(),
            kind: "evidence".into(),
            actor_category: Some("system".into()),
            target_id: Some(e.evidence_id.to_string()),
            summary: format!("Evidence: {}", truncate(&e.summary, 120)),
            detail: Some(format!(
                "{} · {} · {}",
                e.kind.as_str(),
                e.provenance.as_str(),
                e.tool
            )),
            provenance: Some(e.provenance.as_str().into()),
            sort_key: (ts, kind_rank("evidence"), e.evidence_id.to_string()),
        });
    }

    for f in &agent.findings {
        let ts = f.created_at.to_rfc3339();
        entries.push(TimelineEntry {
            id: format!("finding:{}", f.id),
            timestamp: ts.clone(),
            kind: "finding_created".into(),
            actor_category: Some("human".into()),
            target_id: Some(f.id.to_string()),
            summary: format!("Finding ({}): {}", f.kind.as_str(), truncate(&f.body, 100)),
            detail: Some(f.body.clone()),
            provenance: None,
            sort_key: (ts, kind_rank("finding_created"), f.id.to_string()),
        });
        for r in &f.responses {
            let ts = r.created_at.to_rfc3339();
            entries.push(TimelineEntry {
                id: format!("finding_response:{}", r.id),
                timestamp: ts.clone(),
                kind: "finding_responded".into(),
                actor_category: Some(r.author_kind.clone()),
                target_id: Some(f.id.to_string()),
                summary: format!("Finding response: {}", truncate(&r.body, 100)),
                detail: Some(r.body.clone()),
                provenance: None,
                sort_key: (ts, kind_rank("finding_responded"), r.id.to_string()),
            });
        }
    }

    for fe in &agent.finding_events {
        if fe.event.contains("state") {
            let ts = fe.created_at.to_rfc3339();
            entries.push(TimelineEntry {
                id: format!("finding_event:{}", fe.event_id),
                timestamp: ts.clone(),
                kind: "finding_state".into(),
                actor_category: Some("human".into()),
                target_id: Some(fe.finding_id.to_string()),
                summary: format!("Finding state: {}", fe.event),
                detail: fe
                    .to_state
                    .map(|s| s.as_str().to_string())
                    .or_else(|| fe.from_state.map(|s| s.as_str().to_string())),
                provenance: None,
                sort_key: (ts, kind_rank("finding_state"), fe.event_id.to_string()),
            });
        }
    }

    for op in &agent.append_only_ops {
        let (kind, actor) = match op.op_kind.as_str() {
            OP_HUMAN_OBSERVATION_ADD => ("observation", op.actor_category.as_str()),
            OP_RUN_AMEND => ("amendment", op.actor_category.as_str()),
            OP_ENTRY_SUPERSEDE => ("supersession", op.actor_category.as_str()),
            OP_ENTRY_REDACT => ("redaction", op.actor_category.as_str()),
            other => (other, op.actor_category.as_str()),
        };
        let ts = op.created_at.to_rfc3339();
        let summary = match op.relationship {
            LedgerRelationship::Observation => {
                format!(
                    "Observation: {}",
                    truncate(op.new_content.as_deref().unwrap_or(""), 100)
                )
            }
            LedgerRelationship::Amended => format!("Amendment: {}", truncate(&op.reason, 100)),
            LedgerRelationship::Superseded => {
                format!("Supersession: {}", truncate(&op.reason, 100))
            }
            LedgerRelationship::Redacted => {
                format!("Redaction (explicit): {}", truncate(&op.reason, 100))
            }
        };
        let detail = match op.relationship {
            LedgerRelationship::Redacted => Some(format!(
                "Reason: {}\nPrior content retained in structured ledger (not shown in ordinary UI).",
                op.reason
            )),
            _ => Some(format!(
                "Reason: {}\nPrior: {}\nNew: {}",
                op.reason,
                op.previous_content.as_deref().unwrap_or("—"),
                op.new_content.as_deref().unwrap_or("—")
            )),
        };
        entries.push(TimelineEntry {
            id: format!("append:{}", op.op_id),
            timestamp: ts.clone(),
            kind: kind.into(),
            actor_category: Some(actor.into()),
            target_id: op.target_id.map(|u| u.to_string()),
            summary,
            detail,
            provenance: None,
            sort_key: (ts, kind_rank(kind), op.op_id.to_string()),
        });
    }

    entries.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    entries
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

/// Deduplicate project roots by project UUID (canonical path preferred).
pub fn dedupe_project_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut by_id: BTreeMap<Uuid, PathBuf> = BTreeMap::new();
    let mut orphans: Vec<PathBuf> = Vec::new();
    for root in roots {
        let canon = canonicalize_existing(&root);
        match resolve_existing_project(Some(&canon)) {
            Ok(r) => {
                by_id.entry(r.project_id).or_insert(canon);
            }
            Err(_) => {
                if !orphans.iter().any(|p| p == &canon) {
                    orphans.push(canon);
                }
            }
        }
    }
    let mut out: Vec<PathBuf> = by_id.into_values().collect();
    out.extend(orphans);
    out.sort();
    out
}

/// Scan roots for `.moraine` projects up to max_depth (read-only).
pub fn scan_project_roots(base: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![(canonicalize_existing(base), 0usize)];
    while let Some((cur, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        if cur.join(MORAINE_DIR).is_dir() {
            found.push(cur);
            continue;
        }
        if let Ok(entries) = fs::read_dir(&cur) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.starts_with('.') || name == "node_modules" || name == "target" {
                        continue;
                    }
                    stack.push((p, depth + 1));
                }
            }
        }
    }
    dedupe_project_roots(found)
}

/// Filter run summaries for UI categories.
pub fn filter_runs<'a>(
    runs: &'a [RunSummary],
    category: Option<&str>,
    open_findings_only: bool,
    has_risks: bool,
    has_questions: bool,
    query: Option<&str>,
) -> Vec<&'a RunSummary> {
    filter_runs_ext(
        runs,
        category,
        open_findings_only,
        has_risks,
        has_questions,
        query,
        None,
    )
}

/// Extended filter with optional capture-coverage constraint.
pub fn filter_runs_ext<'a>(
    runs: &'a [RunSummary],
    category: Option<&str>,
    open_findings_only: bool,
    has_risks: bool,
    has_questions: bool,
    query: Option<&str>,
    capture_coverage: Option<&str>,
) -> Vec<&'a RunSummary> {
    let q = query
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let cov = capture_coverage
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    runs.iter()
        .filter(|r| {
            if open_findings_only && r.open_finding_count == 0 {
                return false;
            }
            if has_risks && r.risk_count == 0 {
                return false;
            }
            if has_questions && r.open_question_count == 0 {
                return false;
            }
            if let Some(ref cov) = cov {
                if r.capture_coverage.to_lowercase() != *cov {
                    return false;
                }
            }
            if let Some(cat) = category {
                match cat {
                    "active" => {
                        if r.lifecycle != "active" {
                            return false;
                        }
                    }
                    "ready" => {
                        if r.lifecycle != "ready_for_review" {
                            return false;
                        }
                    }
                    "recent" => {}
                    _ => {}
                }
            }
            if let Some(ref q) = q {
                let hay = format!(
                    "{} {} {} {}",
                    r.objective, r.run_id, r.record_path, r.lifecycle
                )
                .to_lowercase();
                if !hay.contains(q) {
                    return false;
                }
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        init_project, run_amend, run_checkpoint, run_start, ActorCategory, AmendRequest,
        CheckpointInput, RunStartRequest,
    };
    use std::path::Path;
    use tempfile::tempdir;

    fn setup_run(dir: &Path) -> (Uuid, PathBuf, PathBuf, Uuid) {
        let p = init_project(Some(dir)).unwrap();
        let start = run_start(RunStartRequest {
            objective: "Discovery test objective".into(),
            idempotency_key: "disc-start".into(),
            project: Some(p.project_root.clone()),
            session_id: None,
        })
        .unwrap();
        let cp = run_checkpoint(
            Some(&p.project_root),
            start.run_id,
            &start.content_hash,
            "disc-cp",
            CheckpointInput {
                summary: "All concurrency tests pass.".into(),
                actions: vec![],
                rationales: vec![],
                evidence: vec![],
                risks: vec!["maybe flaky".into()],
                open_questions: vec!["ordering?".into()],
            },
        )
        .unwrap();
        (
            p.project_id,
            p.project_root,
            start.absolute_path,
            cp.op_id.unwrap(),
        )
    }

    #[test]
    fn summarize_and_list_without_mutation() {
        let dir = tempdir().unwrap();
        let (pid, root, md, _) = setup_run(dir.path());
        let before = fs::read(&md).unwrap();
        let side = moraine_sidecar_path(&md);
        let before_side = fs::read(&side).unwrap();

        let proj = summarize_project(&root).unwrap();
        assert_eq!(proj.project_id, pid);
        assert!(proj.available);
        assert_eq!(proj.run_counts.recent, 1);
        assert_eq!(proj.run_counts.active, 1);

        let runs = list_run_summaries(&root, pid);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].checkpoint_count, 1);
        assert_eq!(runs[0].risk_count, 1);
        assert_eq!(runs[0].open_question_count, 1);
        assert_eq!(runs[0].integrity, "current");

        let detail = load_run_detail(&md, pid);
        assert!(detail.is_protocol_run);
        assert!(!detail.timeline.is_empty());
        assert!(detail.timeline.iter().any(|e| e.kind == "checkpoint"));

        assert_eq!(fs::read(&md).unwrap(), before, "markdown must not change");
        assert_eq!(
            fs::read(&side).unwrap(),
            before_side,
            "sidecar must not change"
        );
    }

    #[test]
    fn timeline_shows_amendment_and_original() {
        let dir = tempdir().unwrap();
        let (pid, root, md, cp_id) = setup_run(dir.path());
        run_amend(
            Some(&root),
            // need run id from meta
            load_run_meta_readonly(&md).unwrap().unwrap().run.id,
            AmendRequest {
                target_id: cp_id,
                target_kind: "checkpoint".into(),
                reason: "incomplete".into(),
                new_content: "All concurrency tests, including ordering, pass.".into(),
                actor_category: ActorCategory::Agent,
            },
        )
        .unwrap();

        let detail = load_run_detail(&md, pid);
        let cp_entry = detail
            .timeline
            .iter()
            .find(|e| e.kind == "checkpoint")
            .unwrap();
        assert!(
            cp_entry.detail.as_ref().unwrap().contains("Original claim"),
            "{:?}",
            cp_entry.detail
        );
        assert!(detail.timeline.iter().any(|e| e.kind == "amendment"));
    }

    #[test]
    fn broken_sidecar_represented() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("bad.md");
        fs::write(&md, "# x\n").unwrap();
        let side = moraine_sidecar_path(&md);
        fs::write(&side, "{not json").unwrap();
        let s = summarize_run_path(&md, Uuid::nil());
        assert_eq!(s.integrity, "malformed_sidecar");
        assert!(s.error.is_some());
    }

    #[test]
    fn filter_active_and_search() {
        let dir = tempdir().unwrap();
        let (pid, root, _, _) = setup_run(dir.path());
        let runs = list_run_summaries(&root, pid);
        let active = filter_runs(&runs, Some("active"), false, false, false, None);
        assert_eq!(active.len(), 1);
        let none = filter_runs(&runs, Some("active"), false, false, false, Some("zzzzz"));
        assert!(none.is_empty());
        let with_q = filter_runs(&runs, None, false, false, true, None);
        assert_eq!(with_q.len(), 1);
    }

    #[test]
    fn dedupe_same_project_uuid() {
        let dir = tempdir().unwrap();
        let p = init_project(Some(dir.path())).unwrap();
        let a = p.project_root.clone();
        let b = a.clone();
        let d = dedupe_project_roots(vec![a, b]);
        assert_eq!(d.len(), 1);
    }
}
