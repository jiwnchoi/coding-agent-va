mod agent_session;
mod app_config;
mod indexer;

#[cfg(test)]
mod ts_bindings;

use agent_session::{
    get_agent_session_file_activity, get_agent_session_file_diff, list_agent_sessions,
    manage_agent_session_watch_state, plan_agent_session_watch, start_agent_session_watch,
    stop_agent_session_watch,
};
use app_config::{load_app_settings, save_app_settings};
use indexer::{supported_language_snapshots, ArchitectureGraph, LanguageSupport, WorkspaceIndexer};
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
    manage_agent_session_watch_state(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_indexer_languages,
            index_workspace_graph,
            load_app_settings,
            save_app_settings,
            get_agent_session_file_activity,
            get_agent_session_file_diff,
            list_agent_sessions,
            plan_agent_session_watch,
            start_agent_session_watch,
            stop_agent_session_watch
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
