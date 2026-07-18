use std::path::PathBuf;

use moraine_core::history::{HistoryEntry, HistoryEntryMeta, HistorySource};
use tauri::State;
use uuid::Uuid;

use crate::state::AppState;

#[tauri::command]
pub fn history_list(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryEntryMeta>, String> {
    state
        .history
        .list_meta(&PathBuf::from(path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_get(
    path: String,
    entry_id: String,
    state: State<'_, AppState>,
) -> Result<HistoryEntry, String> {
    let id = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    state
        .history
        .get(&PathBuf::from(path), id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_push(
    path: String,
    content: String,
    label: Option<String>,
    state: State<'_, AppState>,
) -> Result<HistoryEntry, String> {
    state
        .history
        .push(
            &PathBuf::from(path),
            &content,
            HistorySource::Manual,
            label,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_restore_content(
    path: String,
    entry_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let id = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    state
        .history
        .restore_content(&PathBuf::from(path), id)
        .map_err(|e| e.to_string())
}
