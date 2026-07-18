use std::path::PathBuf;

use moraine_core::{
    ensure_run_meta, moraine_sidecar_path, record_decision, review_snapshot, DecisionKind,
    Document, Error as CoreError, ReviewStateKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionDto {
    pub id: String,
    pub decision: String,
    pub reviewer_label: String,
    pub reason: Option<String>,
    pub created_at: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReviewDto {
    pub run_id: String,
    pub content_hash: String,
    pub review_state: String,
    pub decision_current: bool,
    pub decision_count: usize,
    pub latest: Option<DecisionDto>,
    pub sidecar: String,
    pub initialized: bool,
}

fn map_err(e: CoreError) -> String {
    // Structured string so the UI can detect revision conflicts.
    match &e {
        CoreError::RevisionConflict { expected, actual } => {
            format!("revision_conflict: expected {expected}, actual {actual}")
        }
        CoreError::LedgerBusy(msg) => format!("ledger_busy: {msg}"),
        other => other.to_string(),
    }
}

fn state_str(s: ReviewStateKind) -> &'static str {
    match s {
        ReviewStateKind::Unreviewed => "unreviewed",
        ReviewStateKind::Approved => "approved",
        ReviewStateKind::ChangesRequested => "changes_requested",
        ReviewStateKind::Rejected => "rejected",
        ReviewStateKind::Stale => "stale",
    }
}

fn decision_dto(d: &moraine_core::ReviewDecision) -> DecisionDto {
    DecisionDto {
        id: d.id.to_string(),
        decision: d.decision.as_str().into(),
        reviewer_label: d.reviewer_label.clone(),
        reason: d.reason.clone(),
        created_at: d.created_at.to_rfc3339(),
        content_hash: d.content_hash.clone(),
    }
}

fn dto_from_snap(path: &std::path::Path, snap: &moraine_core::ReviewSnapshot) -> RunReviewDto {
    RunReviewDto {
        run_id: snap.run_id.to_string(),
        content_hash: snap.content_hash.clone(),
        review_state: state_str(snap.state).into(),
        decision_current: snap.decision_current,
        decision_count: snap.decision_count,
        latest: snap.latest.as_ref().map(decision_dto),
        sidecar: moraine_sidecar_path(path).display().to_string(),
        initialized: snap.initialized,
    }
}

/// Load review state; creates the ledger on first open (deliberate GUI init).
#[tauri::command]
pub fn get_run_review(path: String) -> Result<RunReviewDto, String> {
    let path = PathBuf::from(path);
    let markdown = Document::read_file(&path).map_err(map_err)?;
    let meta = ensure_run_meta(&path).map_err(map_err)?;
    let snap = review_snapshot(&meta, &markdown);
    Ok(dto_from_snap(&path, &snap))
}

#[tauri::command]
pub fn record_run_decision(
    path: String,
    decision: String,
    reviewer_label: String,
    reason: Option<String>,
    expected_content_hash: String,
) -> Result<RunReviewDto, String> {
    let kind = DecisionKind::parse(&decision)
        .ok_or_else(|| "invalid decision (approved|changes_requested|rejected)".to_string())?;
    if reviewer_label.trim().is_empty() {
        return Err("reviewer label required".into());
    }
    if expected_content_hash.trim().is_empty() {
        return Err("expectedContentHash required (save first)".into());
    }
    let path = PathBuf::from(path);
    let (_meta, _recorded, snap) = record_decision(
        &path,
        kind,
        reviewer_label.trim(),
        reason,
        expected_content_hash.trim(),
    )
    .map_err(map_err)?;
    Ok(dto_from_snap(&path, &snap))
}

#[tauri::command]
pub fn ensure_run_id(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    let meta = ensure_run_meta(&path).map_err(map_err)?;
    Ok(meta.run.id.to_string())
}
