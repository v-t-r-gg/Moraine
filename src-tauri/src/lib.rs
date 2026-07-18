mod commands;
mod state;

use state::AppState;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    let app_state = AppState::new().expect("failed to initialize Moraine paths");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state)
        .setup(|app| {
            AppState::start_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::app_info,
            commands::take_startup_path,
            commands::open_document,
            commands::get_document,
            commands::set_document_content,
            commands::save_document,
            commands::reload_document,
            commands::close_document,
            commands::list_open_documents,
            commands::read_file,
            commands::write_file,
            commands::history_list,
            commands::history_get,
            commands::history_push,
            commands::history_restore_content,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Moraine");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
