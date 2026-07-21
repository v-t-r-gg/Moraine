//! Host commands for human review findings on agent runs.

use std::path::PathBuf;

use moraine_core::{
    change_finding_state_at_path, create_finding_at_path, get_finding_at_path,
    list_findings_at_path, load_run_checkpoints_detail, CreateFindingRequest, Error as CoreError,
    FindingKind, FindingState,
};
use uuid::Uuid;

fn map_err(e: CoreError) -> String {
    match &e {
        CoreError::RevisionConflict { expected, actual } => {
            format!("revision_conflict: expected {expected}, actual {actual}")
        }
        CoreError::LedgerBusy(msg) => format!("ledger_busy: {msg}"),
        CoreError::FindingNotFound { id } => format!("finding_not_found: {id}"),
        CoreError::InvalidFinding { message } => format!("invalid_finding: {message}"),
        other => other.to_string(),
    }
}

fn parse_kind(s: &str) -> Result<FindingKind, String> {
    FindingKind::parse(s).ok_or_else(|| {
        format!(
            "invalid_finding: unknown kind {s:?}; expected clarification, inconsistency, \
             missing_evidence, risk_concern, factual_correction, or other"
        )
    })
}

fn parse_state(s: &str) -> Result<FindingState, String> {
    FindingState::parse(s).ok_or_else(|| {
        format!("invalid_finding: unknown state {s:?}; expected open, addressed, or archived")
    })
}

fn parse_uuid(s: &str, label: &str) -> Result<Uuid, String> {
    Uuid::parse_str(s.trim()).map_err(|_| format!("invalid {label}"))
}

#[tauri::command]
pub fn create_finding_cmd(
    path: String,
    checkpoint_op_id: String,
    kind: String,
    body: String,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let checkpoint_op_id = parse_uuid(&checkpoint_op_id, "checkpointOpId")?;
    let kind = parse_kind(&kind)?;
    let result = create_finding_at_path(
        &path,
        CreateFindingRequest {
            kind,
            body,
            checkpoint_op_id,
        },
    )
    .map_err(map_err)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_findings_cmd(path: String, open_only: Option<bool>) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let open_only = open_only.unwrap_or(false);
    let items = list_findings_at_path(&path, open_only).map_err(map_err)?;
    serde_json::to_value(items).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_finding_cmd(path: String, finding_id: String) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let finding_id = parse_uuid(&finding_id, "findingId")?;
    let detail = get_finding_at_path(&path, finding_id).map_err(map_err)?;
    serde_json::to_value(detail).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn change_finding_state_cmd(
    path: String,
    finding_id: String,
    state: String,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let finding_id = parse_uuid(&finding_id, "findingId")?;
    let state = parse_state(&state)?;
    let result = change_finding_state_at_path(&path, finding_id, state).map_err(map_err)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_run_checkpoints_cmd(path: String) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let detail = load_run_checkpoints_detail(&path).map_err(map_err)?;
    serde_json::to_value(detail).map_err(|e| e.to_string())
}
