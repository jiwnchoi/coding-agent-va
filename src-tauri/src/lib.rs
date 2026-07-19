mod agent_session;
mod app_config;
mod indexer;
mod shared;

#[cfg(test)]
mod ts_bindings;

use agent_session::{
    describe_agent_session_node, get_agent_session_details, get_agent_session_file_diff,
    list_agent_sessions, manage_agent_session_watch_state, plan_agent_session_watch,
    start_agent_session_watch, stop_agent_session_watch,
};
use app_config::{load_app_settings, save_app_settings};
use indexer::{
    supported_language_snapshots, ArchitectureGraph, LanguageSupport, WorkspaceIndexState,
    WorkspaceIndexer,
};
use shared::logger::{clear_logs, get_logs, write_log};
use std::path::PathBuf;
use std::time::Instant;

#[tauri::command]
fn list_indexer_languages() -> Vec<LanguageSupport> {
    supported_language_snapshots()
}

#[tauri::command]
async fn index_workspace_graph(
    state: tauri::State<'_, WorkspaceIndexState>,
    workspace_path: String,
) -> Result<ArchitectureGraph, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let started_at = Instant::now();
        let context = Some(std::collections::BTreeMap::from([(
            String::from("workspacePath"),
            workspace_path.clone(),
        )]));
        shared::logger::Logger::log(
            shared::logger::LogLevel::Info,
            "Indexing workspace graph",
            context,
        )?;
        let index = state.snapshot(&PathBuf::from(&workspace_path))?;
        let graph = WorkspaceIndexer::index_cached(&index);
        shared::logger::Logger::log(
            shared::logger::LogLevel::Info,
            "Workspace graph indexed",
            Some(std::collections::BTreeMap::from([
                (String::from("workspacePath"), workspace_path),
                (
                    String::from("durationMs"),
                    started_at.elapsed().as_millis().to_string(),
                ),
                (String::from("nodeCount"), graph.nodes.len().to_string()),
                (String::from("edgeCount"), graph.edges.len().to_string()),
            ])),
        )?;
        Ok(graph)
    })
    .await
    .map_err(|error| format!("workspace indexing task failed: {error}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ =
        shared::logger::Logger::log(shared::logger::LogLevel::Info, "Application started", None);
    let builder = manage_agent_session_watch_state(
        tauri::Builder::default().manage(WorkspaceIndexState::default()),
    );

    #[cfg(debug_assertions)]
    let builder = if std::env::var_os("TAURI_MCP_ENABLED").is_some() {
        builder.plugin(tauri_plugin_mcp_bridge::init())
    } else {
        builder
    };

    builder
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
            get_agent_session_file_diff,
            get_agent_session_details,
            list_agent_sessions,
            plan_agent_session_watch,
            start_agent_session_watch,
            stop_agent_session_watch
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
