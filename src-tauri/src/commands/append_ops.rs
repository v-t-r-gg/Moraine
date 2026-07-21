//! Host commands for append-only ledger ops (observations, amend, supersede, redact).

use std::path::PathBuf;

use moraine_core::{
    entry_redact_at_path, entry_supersede_at_path, human_observation_add_at_path,
    list_append_ops_at_path, run_amend_at_path, ActorCategory, AmendRequest, Error as CoreError,
    HumanObservationRequest, RedactRequest, SupersedeRequest,
};
use uuid::Uuid;

fn map_err(e: CoreError) -> String {
    match &e {
        CoreError::FindingNotFound { id } => format!("not_found: {id}"),
        CoreError::InvalidFinding { message } => format!("invalid_op: {message}"),
        CoreError::LedgerBusy(msg) => format!("ledger_busy: {msg}"),
        other => other.to_string(),
    }
}

fn parse_actor(s: &str) -> Result<ActorCategory, String> {
    ActorCategory::parse(s).ok_or_else(|| {
        format!("invalid_op: unknown actorCategory {s:?}; expected human, agent, or system")
    })
}

fn parse_uuid(s: &str, label: &str) -> Result<Uuid, String> {
    Uuid::parse_str(s.trim()).map_err(|_| format!("invalid {label}"))
}

#[tauri::command]
pub fn list_append_ops_cmd(path: String) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let ops = list_append_ops_at_path(&path).map_err(map_err)?;
    serde_json::to_value(ops).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn human_observation_add_cmd(
    path: String,
    body: String,
    reason: String,
    target_id: Option<String>,
    target_kind: Option<String>,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let target_id = match target_id {
        Some(s) if !s.trim().is_empty() => Some(parse_uuid(&s, "targetId")?),
        _ => None,
    };
    let result = human_observation_add_at_path(
        &path,
        HumanObservationRequest {
            body,
            reason,
            target_id,
            target_kind,
        },
    )
    .map_err(map_err)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn run_amend_cmd(
    path: String,
    target_id: String,
    target_kind: String,
    reason: String,
    new_content: String,
    actor_category: String,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let result = run_amend_at_path(
        &path,
        AmendRequest {
            target_id: parse_uuid(&target_id, "targetId")?,
            target_kind,
            reason,
            new_content,
            actor_category: parse_actor(&actor_category)?,
        },
    )
    .map_err(map_err)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn entry_supersede_cmd(
    path: String,
    target_id: String,
    target_kind: String,
    reason: String,
    new_content: String,
    actor_category: String,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let result = entry_supersede_at_path(
        &path,
        SupersedeRequest {
            target_id: parse_uuid(&target_id, "targetId")?,
            target_kind,
            reason,
            new_content,
            actor_category: parse_actor(&actor_category)?,
        },
    )
    .map_err(map_err)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn entry_redact_cmd(
    path: String,
    target_id: String,
    target_kind: String,
    reason: String,
    actor_category: String,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(path);
    let result = entry_redact_at_path(
        &path,
        RedactRequest {
            target_id: parse_uuid(&target_id, "targetId")?,
            target_kind,
            reason,
            actor_category: parse_actor(&actor_category)?,
        },
    )
    .map_err(map_err)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}
