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
            commands::load_comments,
            commands::create_annotation_cmd,
            commands::update_annotation_cmd,
            commands::resolve_annotation_cmd,
            commands::reopen_annotation_cmd,
            commands::begin_accept_suggestion_cmd,
            commands::complete_accept_suggestion_cmd,
            commands::cancel_accept_suggestion_cmd,
            commands::reject_suggestion_cmd,
            commands::reconcile_session_annotations_cmd,
            commands::comments_sidecar_path_cmd,
            commands::get_run_review,
            commands::record_run_decision,
            commands::ensure_run_id,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Moraine");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
