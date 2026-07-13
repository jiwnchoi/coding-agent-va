mod agent_session;
mod app_config;
mod indexer;
mod shared;

#[cfg(test)]
mod ts_bindings;

use agent_session::{
    describe_agent_session_node, get_agent_session_file_activity, get_agent_session_file_diff,
    list_agent_sessions, manage_agent_session_watch_state, plan_agent_session_watch,
    start_agent_session_watch, stop_agent_session_watch,
};
use app_config::{load_app_settings, save_app_settings};
use indexer::{supported_language_snapshots, ArchitectureGraph, LanguageSupport, WorkspaceIndexer};
use shared::logger::{clear_logs, get_logs, write_log};
use std::path::PathBuf;

#[tauri::command]
fn list_indexer_languages() -> Vec<LanguageSupport> {
    supported_language_snapshots()
}

#[tauri::command]
async fn index_workspace_graph(workspace_path: String) -> Result<ArchitectureGraph, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let context = Some(std::collections::BTreeMap::from([(
            String::from("workspacePath"),
            workspace_path.clone(),
        )]));
        shared::logger::Logger::log(
            shared::logger::LogLevel::Info,
            "Indexing workspace graph",
            context,
        )?;
        WorkspaceIndexer::index(&PathBuf::from(workspace_path))
    })
    .await
    .map_err(|error| format!("workspace indexing task failed: {error}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ =
        shared::logger::Logger::log(shared::logger::LogLevel::Info, "Application started", None);
    manage_agent_session_watch_state(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_indexer_languages,
            index_workspace_graph,
            load_app_settings,
            save_app_settings,
            write_log,
            get_logs,
            clear_logs,
            describe_agent_session_node,
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
