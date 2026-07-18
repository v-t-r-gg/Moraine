use tauri::State;

use crate::state::AppState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub data_dir: String,
    pub history_dir: String,
    pub config_dir: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        name: "Moraine".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: state.paths.data_dir.display().to_string(),
        history_dir: state.paths.history_dir.display().to_string(),
        config_dir: state.paths.config_dir.display().to_string(),
    }
}

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Moraine is ready, {name}!")
}
