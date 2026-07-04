mod indexer;
mod session_watch;

use indexer::{supported_language_snapshots, ArchitectureGraph, LanguageSupport, WorkspaceIndexer};
use session_watch::{
    get_codex_session_file_activity, list_codex_sessions, manage_session_watch_state,
    plan_codex_session_watch, start_codex_session_watch, stop_codex_session_watch,
};
use std::path::PathBuf;

#[tauri::command]
fn list_indexer_languages() -> Vec<LanguageSupport> {
    supported_language_snapshots()
}

#[tauri::command]
fn index_workspace_graph(workspace_path: String) -> Result<ArchitectureGraph, String> {
    WorkspaceIndexer::index(&PathBuf::from(workspace_path))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    manage_session_watch_state(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_indexer_languages,
            index_workspace_graph,
            get_codex_session_file_activity,
            list_codex_sessions,
            plan_codex_session_watch,
            start_codex_session_watch,
            stop_codex_session_watch
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
