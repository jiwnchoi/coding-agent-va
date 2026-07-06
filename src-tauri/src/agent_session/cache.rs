use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use super::file_system::{file_updated_at_ms, resolve_existing_dir};
use super::git::read_git_index_updated_at_ms;
use super::paths::normalize_absolute_activity_path;
use super::protocols::AgentSessionProtocol;
use super::state::{
    AgentSessionWatchState, FileActivityCacheEntry, FileActivityCacheKey, SessionListCacheKey,
};
use super::types::{
    AgentSessionFileActivity, AgentSessionProvider, AgentSessionSummary, SessionWatchPlan,
};
use super::watch::build_session_watch_plan;

pub(crate) fn create_watch_plan_cached(
    state: &AgentSessionWatchState,
    protocol: &dyn AgentSessionProtocol,
    runtime_home: &Path,
) -> Result<SessionWatchPlan, String> {
    let runtime_home = resolve_existing_dir(runtime_home)?;
    let sessions = list_sessions_for_resolved_runtime_home_cached(state, protocol, &runtime_home);

    Ok(build_session_watch_plan(
        protocol.provider(),
        protocol.watch_roots(&runtime_home),
        runtime_home,
        &sessions,
    ))
}

pub(crate) fn list_sessions_cached(
    state: &AgentSessionWatchState,
    protocol: &dyn AgentSessionProtocol,
    runtime_home: &Path,
) -> Result<Vec<AgentSessionSummary>, String> {
    let runtime_home = resolve_existing_dir(runtime_home)?;

    Ok(list_sessions_for_resolved_runtime_home_cached(
        state,
        protocol,
        &runtime_home,
    ))
}

pub(crate) fn read_file_activity_cached(
    state: &AgentSessionWatchState,
    provider: AgentSessionProvider,
    transcript_path: &Path,
    cwd: Option<&str>,
) -> Result<AgentSessionFileActivity, String> {
    let key = file_activity_cache_key(provider, transcript_path, cwd);
    let transcript_updated_at_ms = file_updated_at_ms(transcript_path);
    let git_index_updated_at_ms = cwd.and_then(read_git_index_updated_at_ms);

    if let Ok(cache) = state.file_activities.lock() {
        if let Some(entry) = cache.get(&key) {
            if entry.transcript_updated_at_ms == transcript_updated_at_ms
                && entry.git_index_updated_at_ms == git_index_updated_at_ms
            {
                return Ok(entry.activity.clone());
            }
        }
    }

    let activity = provider
        .protocol()
        .read_file_activity(transcript_path, cwd)?;
    if let Ok(mut cache) = state.file_activities.lock() {
        cache.insert(
            key,
            FileActivityCacheEntry {
                transcript_updated_at_ms,
                git_index_updated_at_ms,
                activity: activity.clone(),
            },
        );
    }

    Ok(activity)
}

pub(crate) fn invalidate_watch_caches(
    app: &AppHandle,
    provider: AgentSessionProvider,
    runtime_home: &Path,
    changed_paths: &[String],
    git_index_paths: &[PathBuf],
) {
    let state = app.state::<AgentSessionWatchState>();
    let git_index_path_set = git_index_paths
        .iter()
        .map(|path| normalize_absolute_activity_path(path))
        .collect::<HashSet<_>>();

    if changed_paths
        .iter()
        .any(|path| !git_index_path_set.contains(path))
    {
        let key = session_list_cache_key(provider, runtime_home);
        if let Ok(mut cache) = state.session_lists.lock() {
            cache.remove(&key);
        }
    }

    let git_index_changed = changed_paths
        .iter()
        .any(|path| git_index_path_set.contains(path));
    let changed_path_set = changed_paths
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    if let Ok(mut cache) = state.file_activities.lock() {
        cache.retain(|key, _| {
            key.provider != provider
                || (!changed_path_set.contains(key.transcript_path.as_str()) && !git_index_changed)
        });
    };
}

fn list_sessions_for_resolved_runtime_home_cached(
    state: &AgentSessionWatchState,
    protocol: &dyn AgentSessionProtocol,
    runtime_home: &Path,
) -> Vec<AgentSessionSummary> {
    let key = session_list_cache_key(protocol.provider(), runtime_home);
    if let Ok(cache) = state.session_lists.lock() {
        if let Some(sessions) = cache.get(&key) {
            return sessions.clone();
        }
    }

    let sessions = protocol.list_sessions(runtime_home);
    if let Ok(mut cache) = state.session_lists.lock() {
        cache.insert(key, sessions.clone());
    }

    sessions
}

fn session_list_cache_key(
    provider: AgentSessionProvider,
    runtime_home: &Path,
) -> SessionListCacheKey {
    SessionListCacheKey {
        provider,
        runtime_home: normalize_absolute_activity_path(runtime_home),
    }
}

fn file_activity_cache_key(
    provider: AgentSessionProvider,
    transcript_path: &Path,
    cwd: Option<&str>,
) -> FileActivityCacheKey {
    FileActivityCacheKey {
        provider,
        transcript_path: normalize_absolute_activity_path(transcript_path),
        cwd: cwd.map(|path| normalize_absolute_activity_path(Path::new(path))),
    }
}
