use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use rayon::prelude::*;
use tauri::{ipc::Channel, AppHandle, State};
use watchexec::Watchexec;

use crate::indexer::WorkspaceIndexState;
use crate::shared::logger::{LogLevel, Logger};

use super::cache::{
    create_watch_plan_cached, invalidate_watch_caches, list_session_candidates_cached,
};
use super::description::{describe_session_node, NodeDescriptionCacheState};
use super::file_system::resolve_existing_dir;
use super::protocols::default_runtime_sources;
use super::protocols::AgentSessionCandidate;
use super::session_details::read_session_details_cached;
use super::session_diff::read_agent_session_file_diff;
use super::state::{AgentSessionWatchState, SessionWatchHandle};
use super::time::now_timestamp_ms;
use super::types::{
    AgentSessionDetails, AgentSessionFileDiff, AgentSessionList,
    AgentSessionNodeDescriptionRequest, AgentSessionNodeDescriptionResponse,
    AgentSessionNodeDescriptionStreamEvent, AgentSessionProvider, AgentSessionSummary,
    SessionWatchEventPayload, SessionWatchPlan, SessionWatchRegistration,
};
use super::watch::{
    emit_session_watch_event, is_relevant_watch_path, normalize_watch_event_path,
    stop_existing_watch, watch_stopped_payload, watched_paths_from_targets,
};

const MAX_SESSION_PAGE_SIZE: usize = 100;

#[tauri::command]
pub async fn get_agent_session_details(
    index_state: State<'_, WorkspaceIndexState>,
    provider: AgentSessionProvider,
    provider_session_id: String,
    transcript_path: String,
    runtime_home: String,
    cwd: Option<String>,
) -> Result<AgentSessionDetails, String> {
    let index_state = index_state.inner().clone();
    run_blocking("session details", move || {
        read_session_details_cached(
            &index_state,
            provider,
            &provider_session_id,
            &PathBuf::from(transcript_path),
            &PathBuf::from(runtime_home),
            cwd.as_deref(),
        )
    })
    .await
}

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
        let (sessions, next_offset) = hydrate_session_page(&candidates, offset, limit, |chunk| {
            sources
                .par_iter()
                .filter(|source| source.available)
                .filter_map(|source| {
                    let source_candidates = chunk
                        .iter()
                        .filter(|item| item.provider == source.provider)
                        .collect::<Vec<_>>();
                    let runtime_home = &source_candidates.first()?.runtime_home;
                    let page_candidates = source_candidates
                        .iter()
                        .map(|item| item.candidate.clone())
                        .collect::<Vec<_>>();
                    let sessions = source
                        .provider
                        .protocol()
                        .hydrate_sessions(runtime_home, &page_candidates);
                    Some(sessions)
                })
                .collect::<Vec<_>>()
                .into_iter()
                .flatten()
                .collect()
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
            has_more: next_offset < candidates.len(),
            next_offset,
        })
    })
    .await
}

struct SessionCandidateSource {
    provider: AgentSessionProvider,
    runtime_home: PathBuf,
    candidate: AgentSessionCandidate,
}

fn hydrate_session_page(
    candidates: &[SessionCandidateSource],
    offset: usize,
    limit: usize,
    mut hydrate_chunk: impl FnMut(&[SessionCandidateSource]) -> Vec<AgentSessionSummary>,
) -> (Vec<AgentSessionSummary>, usize) {
    let mut next_offset = offset.min(candidates.len());
    let mut sessions = Vec::with_capacity(limit);
    while sessions.len() < limit && next_offset < candidates.len() {
        let chunk_end = next_offset
            .saturating_add(limit - sessions.len())
            .min(candidates.len());
        let chunk = &candidates[next_offset..chunk_end];
        let mut hydrated_by_path = hydrate_chunk(chunk)
            .into_iter()
            .map(|session| (PathBuf::from(&session.transcript_path), session))
            .collect::<HashMap<_, _>>();
        sessions.extend(
            chunk
                .iter()
                .filter_map(|item| hydrated_by_path.remove(&item.candidate.transcript_path)),
        );
        next_offset = chunk_end;
    }

    (sessions, next_offset)
}

#[tauri::command]
pub async fn plan_agent_session_watch(
    provider: AgentSessionProvider,
    runtime_home: String,
) -> Result<SessionWatchPlan, String> {
    run_blocking("session watch planning", move || {
        let protocol = provider.protocol();
        create_watch_plan_cached(protocol.as_ref(), &PathBuf::from(runtime_home))
    })
    .await
}

#[tauri::command]
pub async fn get_agent_session_file_diff(
    provider: AgentSessionProvider,
    transcript_path: String,
    file_path: String,
    cwd: Option<String>,
    replay_session: bool,
    start_entry_index: Option<usize>,
    end_entry_index: Option<usize>,
) -> Result<AgentSessionFileDiff, String> {
    run_blocking("session file diff", move || {
        read_agent_session_file_diff(
            provider,
            &PathBuf::from(transcript_path),
            &PathBuf::from(file_path),
            cwd.as_deref(),
            replay_session,
            start_entry_index.zip(end_entry_index),
        )
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
    let plan = run_blocking("session watch startup", move || {
        let protocol = provider.protocol();
        create_watch_plan_cached(protocol.as_ref(), &PathBuf::from(runtime_home))
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
    let wx = Watchexec::new(move |action| {
        let changed_paths = action
            .events
            .iter()
            .flat_map(|event| event.paths().map(|(path, _)| path.to_path_buf()))
            .map(|path| normalize_watch_event_path(&path))
            .filter(|path| is_relevant_watch_path(provider, path, &runtime_home_for_task))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();

        if !changed_paths.is_empty() {
            invalidate_watch_caches(&app_handle, provider, &runtime_home_for_task);

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

    let mut fs_ready = wx.config.fs_ready();
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

    match tokio::time::timeout(Duration::from_secs(5), fs_ready.changed()).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => {
            watch_task.abort();
            return Err("agent session filesystem watcher stopped during startup".to_string());
        }
        Err(_) => {
            watch_task.abort();
            return Err("timed out while registering agent session watch paths".to_string());
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_pages_fill_past_invalid_candidates() {
        let candidates = ["invalid.jsonl", "valid-1.jsonl", "valid-2.jsonl"]
            .into_iter()
            .map(|name| SessionCandidateSource {
                provider: AgentSessionProvider::Pi,
                runtime_home: PathBuf::from("/runtime"),
                candidate: AgentSessionCandidate {
                    transcript_path: PathBuf::from("/runtime").join(name),
                    updated_at_ms: 1,
                },
            })
            .collect::<Vec<_>>();

        let (sessions, next_offset) = hydrate_session_page(&candidates, 0, 2, |chunk| {
            chunk
                .iter()
                .filter(|item| {
                    item.candidate
                        .transcript_path
                        .file_name()
                        .is_some_and(|name| name != "invalid.jsonl")
                })
                .map(|item| AgentSessionSummary {
                    id: item.candidate.transcript_path.display().to_string(),
                    provider: item.provider,
                    provider_session_id: "session".to_string(),
                    provider_label: "Pi Agent".to_string(),
                    title: "Session".to_string(),
                    transcript_path: item.candidate.transcript_path.display().to_string(),
                    cwd: None,
                    runtime_home: item.runtime_home.display().to_string(),
                    updated_at_ms: item.candidate.updated_at_ms,
                })
                .collect()
        });

        assert_eq!(sessions.len(), 2);
        assert_eq!(next_offset, 3);
        assert!(sessions[0].transcript_path.ends_with("valid-1.jsonl"));
        assert!(sessions[1].transcript_path.ends_with("valid-2.jsonl"));
    }
}
