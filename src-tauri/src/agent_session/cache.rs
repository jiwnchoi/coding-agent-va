use std::path::Path;

use tauri::{AppHandle, Manager};

use super::file_system::resolve_existing_dir;
use super::paths::normalize_absolute_activity_path;
use super::protocols::{AgentSessionCandidate, AgentSessionProtocol};
use super::state::{AgentSessionWatchState, SessionListCacheKey};
use super::types::{AgentSessionProvider, SessionWatchPlan};
use super::watch::build_session_watch_plan;

pub(crate) fn create_watch_plan_cached(
    protocol: &dyn AgentSessionProtocol,
    runtime_home: &Path,
) -> Result<SessionWatchPlan, String> {
    let runtime_home = resolve_existing_dir(runtime_home)?;
    Ok(build_session_watch_plan(
        protocol.provider(),
        protocol.watch_roots(&runtime_home),
        runtime_home,
    ))
}

pub(crate) fn list_session_candidates_cached(
    state: &AgentSessionWatchState,
    protocol: &dyn AgentSessionProtocol,
    runtime_home: &Path,
) -> Result<Vec<AgentSessionCandidate>, String> {
    let runtime_home = resolve_existing_dir(runtime_home)?;

    let key = session_list_cache_key(protocol.provider(), &runtime_home);
    if let Ok(cache) = state.session_candidates.lock() {
        if let Some(candidates) = cache.get(&key) {
            return Ok(candidates.clone());
        }
    }

    let candidates = protocol.list_session_candidates(&runtime_home);
    if let Ok(mut cache) = state.session_candidates.lock() {
        cache.insert(key, candidates.clone());
    }

    Ok(candidates)
}

pub(crate) fn invalidate_watch_caches(
    app: &AppHandle,
    provider: AgentSessionProvider,
    runtime_home: &Path,
) {
    let state = app.state::<AgentSessionWatchState>();
    let key = session_list_cache_key(provider, runtime_home);
    if let Ok(mut cache) = state.session_candidates.lock() {
        cache.remove(&key);
    };
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
