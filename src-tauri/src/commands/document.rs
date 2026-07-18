use std::path::PathBuf;

use moraine_core::{DocumentSnapshot, Error as CoreError};
use tauri::State;
use uuid::Uuid;

use crate::state::AppState;

fn map_err(e: CoreError) -> String {
    e.to_string()
}

#[tauri::command]
pub fn open_document(path: String, state: State<'_, AppState>) -> Result<DocumentSnapshot, String> {
    state.open_path(PathBuf::from(path)).map_err(map_err)
}

#[tauri::command]
pub fn get_document(id: String, state: State<'_, AppState>) -> Result<DocumentSnapshot, String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.get_snapshot(id).map_err(map_err)
}

#[tauri::command]
pub fn set_document_content(
    id: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.set_content(id, content).map_err(map_err)
}

#[tauri::command]
pub fn save_document(
    id: String,
    content: Option<String>,
    record_history: Option<bool>,
    state: State<'_, AppState>,
) -> Result<DocumentSnapshot, String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state
        .save(id, content, record_history.unwrap_or(true))
        .map_err(map_err)
}

#[tauri::command]
pub fn reload_document(id: String, state: State<'_, AppState>) -> Result<DocumentSnapshot, String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.reload(id).map_err(map_err)
}

#[tauri::command]
pub fn close_document(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.close(id);
    Ok(())
}

#[tauri::command]
pub fn list_open_documents(state: State<'_, AppState>) -> Vec<DocumentSnapshot> {
    state.list_open()
}

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    moraine_core::Document::read_file(PathBuf::from(path)).map_err(map_err)
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    moraine_core::Document::write_file(PathBuf::from(path), &content).map_err(map_err)
}
