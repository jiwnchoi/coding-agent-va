use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, State};
use watchexec::Watchexec;

use super::cache::{
    create_watch_plan_cached, invalidate_watch_caches, list_sessions_cached,
    read_file_activity_cached,
};
use super::git::read_agent_session_file_diff;
use super::protocols::default_runtime_sources;
use super::state::{AgentSessionWatchState, SessionWatchHandle};
use super::time::now_timestamp_ms;
use super::types::{
    AgentSessionFileActivity, AgentSessionFileDiff, AgentSessionList, AgentSessionProvider,
    SessionWatchEventPayload, SessionWatchPlan, SessionWatchRegistration,
};
use super::watch::{
    emit_session_watch_event, is_relevant_watch_path, normalize_watch_event_path,
    stop_existing_watch, watch_stopped_payload, watched_paths_from_targets,
};

#[tauri::command]
pub fn list_agent_sessions(
    state: State<'_, AgentSessionWatchState>,
    runtime_homes: Option<BTreeMap<String, String>>,
) -> Result<AgentSessionList, String> {
    let sources = default_runtime_sources(runtime_homes.as_ref());
    let mut sessions = Vec::new();

    for source in &sources {
        if !source.available {
            continue;
        }

        let protocol = source.provider.protocol();
        sessions.extend(list_sessions_cached(
            &state,
            protocol.as_ref(),
            &PathBuf::from(&source.runtime_home),
        )?);
    }

    sessions.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));

    Ok(AgentSessionList { sources, sessions })
}

#[tauri::command]
pub fn plan_agent_session_watch(
    state: State<'_, AgentSessionWatchState>,
    provider: AgentSessionProvider,
    runtime_home: String,
) -> Result<SessionWatchPlan, String> {
    let protocol = provider.protocol();
    create_watch_plan_cached(&state, protocol.as_ref(), &PathBuf::from(runtime_home))
}

#[tauri::command]
pub fn get_agent_session_file_activity(
    state: State<'_, AgentSessionWatchState>,
    provider: AgentSessionProvider,
    transcript_path: String,
    cwd: Option<String>,
    hide_committed_files: bool,
) -> Result<AgentSessionFileActivity, String> {
    read_file_activity_cached(
        &state,
        provider,
        &PathBuf::from(transcript_path),
        cwd.as_deref(),
        hide_committed_files,
    )
}

#[tauri::command]
pub fn get_agent_session_file_diff(
    file_path: String,
    cwd: Option<String>,
) -> Result<AgentSessionFileDiff, String> {
    read_agent_session_file_diff(&PathBuf::from(file_path), cwd.as_deref())
}

#[tauri::command]
pub async fn start_agent_session_watch(
    app: AppHandle,
    state: State<'_, AgentSessionWatchState>,
    provider: AgentSessionProvider,
    runtime_home: String,
) -> Result<SessionWatchRegistration, String> {
    let protocol = provider.protocol();
    let plan = create_watch_plan_cached(&state, protocol.as_ref(), &PathBuf::from(runtime_home))?;
    let watch_id = plan.watch_id.clone();
    let runtime_home_path = PathBuf::from(&plan.runtime_home);
    let git_index_paths = plan
        .git_index_paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let watch_paths = watched_paths_from_targets(&plan.watch_targets);

    stop_existing_watch(&state, &watch_id);

    emit_session_watch_event(
        &app,
        SessionWatchEventPayload {
            watch_id: watch_id.clone(),
            provider,
            runtime_home: plan.runtime_home.clone(),
            changed_paths: watch_paths
                .iter()
                .map(PathBuf::from)
                .map(|path| path.display().to_string())
                .collect(),
            event_tags: vec!["watch_started".to_string()],
            timestamp_ms: now_timestamp_ms(),
        },
    );

    let app_handle = app.clone();
    let watch_id_for_task = watch_id.clone();
    let runtime_home_for_task = runtime_home_path.clone();
    let git_index_paths_for_task = git_index_paths.clone();
    let wx = Watchexec::new(move |action| {
        let changed_paths = action
            .events
            .iter()
            .flat_map(|event| event.paths().map(|(path, _)| path.to_path_buf()))
            .map(|path| normalize_watch_event_path(&path))
            .filter(|path| {
                is_relevant_watch_path(
                    provider,
                    path,
                    &runtime_home_for_task,
                    &git_index_paths_for_task,
                )
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();

        if !changed_paths.is_empty() {
            invalidate_watch_caches(
                &app_handle,
                provider,
                &runtime_home_for_task,
                &changed_paths,
                &git_index_paths_for_task,
            );

            let event_tags = action
                .events
                .iter()
                .flat_map(|event| event.tags.iter().map(|tag| format!("{tag:?}")))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            emit_session_watch_event(
                &app_handle,
                SessionWatchEventPayload {
                    watch_id: watch_id_for_task.clone(),
                    provider,
                    runtime_home: runtime_home_for_task.display().to_string(),
                    changed_paths,
                    event_tags,
                    timestamp_ms: now_timestamp_ms(),
                },
            );
        }

        action
    })
    .map_err(|error| format!("failed to create watchexec watcher: {error}"))?;

    wx.config.throttle(Duration::from_millis(500));
    wx.config.pathset(watch_paths.clone());

    let watch_id_for_runtime_error = watch_id.clone();
    let watch_task = tauri::async_runtime::spawn(async move {
        if let Err(error) = wx.main().await {
            emit_session_watch_event(
                &app,
                SessionWatchEventPayload {
                    watch_id: watch_id_for_runtime_error.clone(),
                    provider,
                    runtime_home: runtime_home_path.display().to_string(),
                    changed_paths: Vec::new(),
                    event_tags: vec![format!("watch_error:{error}")],
                    timestamp_ms: now_timestamp_ms(),
                },
            );
        }
    });

    state
        .watches
        .lock()
        .map_err(|_| "failed to lock session watch state".to_string())?
        .insert(watch_id.clone(), SessionWatchHandle { task: watch_task });

    Ok(SessionWatchRegistration {
        watch_id,
        provider: plan.provider,
        runtime_home: plan.runtime_home,
        watch_targets: plan.watch_targets,
        git_index_paths: plan.git_index_paths,
    })
}

#[tauri::command]
pub fn stop_agent_session_watch(
    app: AppHandle,
    state: State<'_, AgentSessionWatchState>,
    watch_id: String,
) -> Result<bool, String> {
    let removed = state
        .watches
        .lock()
        .map_err(|_| "failed to lock session watch state".to_string())?
        .remove(&watch_id)
        .is_some();

    if removed {
        emit_session_watch_event(&app, watch_stopped_payload(watch_id));
    }

    Ok(removed)
}

pub fn manage_agent_session_watch_state(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.manage(AgentSessionWatchState::default())
}
