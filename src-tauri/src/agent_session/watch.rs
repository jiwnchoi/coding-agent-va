use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter};
use watchexec::WatchedPath;

use super::git::collect_git_index_paths_from_sessions;
use super::paths::normalize_absolute_activity_path;
use super::protocols::provider_key;
use super::state::AgentSessionWatchState;
use super::time::now_timestamp_ms;
use super::types::{
    AgentSessionProvider, AgentSessionSummary, SessionWatchEventPayload, SessionWatchPlan,
    SessionWatchTarget,
};

pub(crate) const AGENT_SESSION_WATCH_EVENT: &str = "agent-session-watch-event";

pub(crate) fn build_session_watch_plan(
    provider: AgentSessionProvider,
    watch_roots: Vec<(PathBuf, bool, String)>,
    runtime_home: PathBuf,
    sessions: &[AgentSessionSummary],
) -> SessionWatchPlan {
    let git_index_paths = collect_git_index_paths_from_sessions(sessions);
    let mut watch_targets = Vec::new();

    for (target_path, recursive, reason) in watch_roots {
        push_watch_target(
            &mut watch_targets,
            existing_or_parent(&target_path, &runtime_home),
            recursive,
            target_path.exists(),
            reason,
        );
    }

    for git_index_path in &git_index_paths {
        push_watch_target(
            &mut watch_targets,
            existing_or_parent(git_index_path, &runtime_home),
            false,
            git_index_path.exists(),
            "watch git index updates".to_string(),
        );
    }

    SessionWatchPlan {
        watch_id: watch_id_for_provider_path(provider, &runtime_home),
        provider,
        runtime_home: runtime_home.display().to_string(),
        watch_targets,
        git_index_paths: git_index_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
    }
}

pub(crate) fn watched_paths_from_targets(targets: &[SessionWatchTarget]) -> Vec<WatchedPath> {
    targets
        .iter()
        .map(|target| {
            let path = PathBuf::from(&target.path);
            if target.recursive {
                WatchedPath::recursive(path)
            } else {
                WatchedPath::non_recursive(path)
            }
        })
        .collect()
}

pub(crate) fn push_watch_target(
    targets: &mut Vec<SessionWatchTarget>,
    path: PathBuf,
    recursive: bool,
    exists: bool,
    reason: String,
) {
    let path_string = path.display().to_string();
    if let Some(target) = targets.iter_mut().find(|target| target.path == path_string) {
        target.recursive |= recursive;
        target.exists |= exists;
        if !target.reason.contains(&reason) {
            target.reason = format!("{}, {reason}", target.reason);
        }
        return;
    }

    targets.push(SessionWatchTarget {
        path: path_string,
        recursive,
        exists,
        reason,
    });
}

pub(crate) fn is_relevant_watch_path(
    provider: AgentSessionProvider,
    path: &Path,
    runtime_home: &Path,
    git_index_paths: &[PathBuf],
) -> bool {
    provider
        .protocol()
        .is_relevant_session_path(path, runtime_home)
        || git_index_paths.iter().any(|index_path| path == index_path)
}

pub(crate) fn normalize_watch_event_path(path: &Path) -> PathBuf {
    PathBuf::from(normalize_absolute_activity_path(path))
}

pub(crate) fn stop_existing_watch(state: &AgentSessionWatchState, watch_id: &str) {
    if let Ok(mut watches) = state.watches.lock() {
        watches.remove(watch_id);
    }
}

pub(crate) fn emit_session_watch_event(app: &AppHandle, payload: SessionWatchEventPayload) {
    let _ = app.emit(AGENT_SESSION_WATCH_EVENT, payload);
}

pub(crate) fn watch_stopped_payload(watch_id: String) -> SessionWatchEventPayload {
    SessionWatchEventPayload {
        watch_id,
        provider: AgentSessionProvider::Codex,
        runtime_home: String::new(),
        changed_paths: Vec::new(),
        event_tags: vec!["watch_stopped".to_string()],
        timestamp_ms: now_timestamp_ms(),
    }
}

fn existing_or_parent(target_path: &Path, runtime_home_path: &Path) -> PathBuf {
    if target_path.exists() {
        return target_path.to_path_buf();
    }

    target_path
        .ancestors()
        .find(|candidate| candidate.exists())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| runtime_home_path.to_path_buf())
}

fn watch_id_for_provider_path(provider: AgentSessionProvider, runtime_home_path: &Path) -> String {
    format!(
        "{}-session-watch:{}",
        provider_key(provider),
        runtime_home_path.display()
    )
}
