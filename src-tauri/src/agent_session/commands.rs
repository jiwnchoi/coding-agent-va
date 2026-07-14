use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use rayon::prelude::*;
use tauri::{ipc::Channel, AppHandle, State};
use watchexec::Watchexec;

use crate::shared::logger::{LogLevel, Logger};

use super::cache::{
    cache_loaded_sessions, create_watch_plan_cached, invalidate_watch_caches,
    list_session_candidates_cached, read_file_activity_cached,
};
use super::description::{describe_session_node, NodeDescriptionCacheState};
use super::file_system::resolve_existing_dir;
use super::git::read_agent_session_file_diff;
use super::protocols::default_runtime_sources;
use super::protocols::AgentSessionCandidate;
use super::state::{AgentSessionWatchState, SessionWatchHandle};
use super::time::now_timestamp_ms;
use super::types::{
    AgentSessionFileActivity, AgentSessionFileDiff, AgentSessionList,
    AgentSessionNodeDescriptionRequest, AgentSessionNodeDescriptionResponse,
    AgentSessionNodeDescriptionStreamEvent, AgentSessionProvider, SessionWatchEventPayload,
    SessionWatchPlan, SessionWatchRegistration,
};
use super::watch::{
    emit_session_watch_event, is_relevant_watch_path, normalize_watch_event_path,
    stop_existing_watch, watch_stopped_payload, watched_paths_from_targets,
};

const MAX_SESSION_PAGE_SIZE: usize = 100;

#[tauri::command]
pub async fn describe_agent_session_node(
    request: AgentSessionNodeDescriptionRequest,
    cache: State<'_, NodeDescriptionCacheState>,
    on_event: Channel<AgentSessionNodeDescriptionStreamEvent>,
) -> Result<AgentSessionNodeDescriptionResponse, String> {
    let cache = cache.inner().clone();
    run_blocking("session description", move || {
        describe_session_node(request, &cache, |event| {
            on_event
                .send(event)
                .map_err(|error| format!("failed to stream description: {error}"))
        })
    })
    .await
}

#[tauri::command]
pub async fn list_agent_sessions(
    state: State<'_, AgentSessionWatchState>,
    runtime_homes: Option<BTreeMap<String, String>>,
    offset: usize,
    limit: usize,
) -> Result<AgentSessionList, String> {
    let state = state.inner().clone();
    run_blocking("session listing", move || {
        let limit = limit.clamp(1, MAX_SESSION_PAGE_SIZE);
        let sources = default_runtime_sources(runtime_homes.as_ref());
        let mut candidates = sources
            .par_iter()
            .filter(|source| source.available)
            .map(|source| -> Result<Vec<SessionCandidateSource>, String> {
                let protocol = source.provider.protocol();
                let runtime_home = resolve_existing_dir(&PathBuf::from(&source.runtime_home))?;
                Ok(
                    list_session_candidates_cached(&state, protocol.as_ref(), &runtime_home)?
                        .into_iter()
                        .map(|candidate| SessionCandidateSource {
                            provider: source.provider,
                            runtime_home: runtime_home.clone(),
                            candidate,
                        })
                        .collect(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .candidate
                .updated_at_ms
                .cmp(&left.candidate.updated_at_ms)
                .then_with(|| {
                    left.candidate
                        .transcript_path
                        .cmp(&right.candidate.transcript_path)
                })
        });
        let page_end = offset.saturating_add(limit).min(candidates.len());
        let page = candidates
            .get(offset.min(candidates.len())..page_end)
            .unwrap_or_default();
        let session_batches = sources
            .par_iter()
            .filter(|source| source.available)
            .filter_map(|source| {
                let source_page = page
                    .iter()
                    .filter(|item| item.provider == source.provider)
                    .collect::<Vec<_>>();
                let runtime_home = &source_page.first()?.runtime_home;
                let page_candidates = source_page
                    .iter()
                    .map(|item| item.candidate.clone())
                    .collect::<Vec<_>>();
                let sessions = source
                    .provider
                    .protocol()
                    .hydrate_sessions(runtime_home, &page_candidates);
                cache_loaded_sessions(&state, source.provider, runtime_home, &sessions);
                Some(sessions)
            })
            .collect::<Vec<_>>();
        let mut sessions = session_batches.into_iter().flatten().collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            right
                .updated_at_ms
                .cmp(&left.updated_at_ms)
                .then_with(|| left.id.cmp(&right.id))
        });

        let _ = Logger::log(
            LogLevel::Info,
            "Listed agent sessions",
            Some(BTreeMap::from([(
                "count".to_string(),
                sessions.len().to_string(),
            )])),
        );

        Ok(AgentSessionList {
            sources,
            sessions,
            has_more: page_end < candidates.len(),
        })
    })
    .await
}

struct SessionCandidateSource {
    provider: AgentSessionProvider,
    runtime_home: PathBuf,
    candidate: AgentSessionCandidate,
}

#[tauri::command]
pub async fn plan_agent_session_watch(
    state: State<'_, AgentSessionWatchState>,
    provider: AgentSessionProvider,
    runtime_home: String,
) -> Result<SessionWatchPlan, String> {
    let state = state.inner().clone();
    run_blocking("session watch planning", move || {
        let protocol = provider.protocol();
        create_watch_plan_cached(&state, protocol.as_ref(), &PathBuf::from(runtime_home))
    })
    .await
}

#[tauri::command]
pub async fn get_agent_session_file_activity(
    state: State<'_, AgentSessionWatchState>,
    provider: AgentSessionProvider,
    transcript_path: String,
    cwd: Option<String>,
    hide_committed_files: bool,
) -> Result<AgentSessionFileActivity, String> {
    let state = state.inner().clone();
    run_blocking("session file activity", move || {
        read_file_activity_cached(
            &state,
            provider,
            &PathBuf::from(transcript_path),
            cwd.as_deref(),
            hide_committed_files,
        )
    })
    .await
}

#[tauri::command]
pub async fn get_agent_session_file_diff(
    file_path: String,
    cwd: Option<String>,
) -> Result<AgentSessionFileDiff, String> {
    run_blocking("session file diff", move || {
        read_agent_session_file_diff(&PathBuf::from(file_path), cwd.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn start_agent_session_watch(
    app: AppHandle,
    state: State<'_, AgentSessionWatchState>,
    provider: AgentSessionProvider,
    runtime_home: String,
) -> Result<SessionWatchRegistration, String> {
    let state = state.inner().clone();
    let state_for_plan = state.clone();
    let plan = run_blocking("session watch startup", move || {
        let protocol = provider.protocol();
        create_watch_plan_cached(
            &state_for_plan,
            protocol.as_ref(),
            &PathBuf::from(runtime_home),
        )
    })
    .await?;
    let watch_id = plan.watch_id.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        Logger::log(
            LogLevel::Info,
            "Started agent session watcher",
            Some(BTreeMap::from([(
                "provider".to_string(),
                format!("{provider:?}"),
            )])),
        )
    })
    .await;
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
            let error_message = error.to_string();
            let error_message_for_log = error_message.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                Logger::log(
                    LogLevel::Error,
                    "Agent session watcher failed",
                    Some(BTreeMap::from([(
                        "error".to_string(),
                        error_message_for_log,
                    )])),
                )
            })
            .await;
            emit_session_watch_event(
                &app,
                SessionWatchEventPayload {
                    watch_id: watch_id_for_runtime_error.clone(),
                    provider,
                    runtime_home: runtime_home_path.display().to_string(),
                    changed_paths: Vec::new(),
                    event_tags: vec![format!("watch_error:{error_message}")],
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
    builder
        .manage(AgentSessionWatchState::default())
        .manage(NodeDescriptionCacheState::default())
}

async fn run_blocking<T, F>(task_name: &'static str, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("{task_name} task failed: {error}"))?
}
