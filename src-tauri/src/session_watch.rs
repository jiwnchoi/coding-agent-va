use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, State};
use watchexec::Watchexec;

const SESSION_WATCH_EVENT: &str = "codex-session-watch-event";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchTarget {
    pub path: String,
    pub recursive: bool,
    pub exists: bool,
    pub reason: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchPlan {
    pub watch_id: String,
    pub runtime_home: String,
    pub watch_targets: Vec<SessionWatchTarget>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchRegistration {
    pub watch_id: String,
    pub runtime_home: String,
    pub watch_targets: Vec<SessionWatchTarget>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchEventPayload {
    pub watch_id: String,
    pub runtime_home: String,
    pub changed_paths: Vec<String>,
    pub event_tags: Vec<String>,
    pub timestamp_ms: u64,
}

#[derive(Default)]
pub struct SessionWatchState {
    watches: Mutex<HashMap<String, SessionWatchHandle>>,
}

struct SessionWatchHandle {
    task: JoinHandle<()>,
}

impl Drop for SessionWatchHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn manage_session_watch_state(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.manage(SessionWatchState::default())
}

#[tauri::command]
pub fn plan_codex_session_watch(runtime_home: String) -> Result<SessionWatchPlan, String> {
    create_watch_plan(&runtime_home)
}

#[tauri::command]
pub fn start_codex_session_watch(
    app: AppHandle,
    state: State<'_, SessionWatchState>,
    runtime_home: String,
) -> Result<SessionWatchRegistration, String> {
    let plan = create_watch_plan(&runtime_home)?;
    let watch_id = plan.watch_id.clone();
    let runtime_home_path = PathBuf::from(&plan.runtime_home);
    let watch_paths = plan
        .watch_targets
        .iter()
        .map(|target| PathBuf::from(&target.path))
        .collect::<Vec<_>>();

    stop_existing_watch(&state, &watch_id);

    emit_session_watch_event(
        &app,
        SessionWatchEventPayload {
            watch_id: watch_id.clone(),
            runtime_home: plan.runtime_home.clone(),
            changed_paths: watch_paths
                .iter()
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
            .filter(|path| is_relevant_session_path(path, &runtime_home_for_task))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();

        if !changed_paths.is_empty() {
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

    wx.config.pathset(watch_paths.clone());

    let watch_id_for_runtime_error = watch_id.clone();
    let watch_task = tauri::async_runtime::spawn(async move {
        if let Err(error) = wx.main().await {
            emit_session_watch_event(
                &app,
                SessionWatchEventPayload {
                    watch_id: watch_id_for_runtime_error.clone(),
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
        runtime_home: plan.runtime_home,
        watch_targets: plan.watch_targets,
    })
}

#[tauri::command]
pub fn stop_codex_session_watch(
    app: AppHandle,
    state: State<'_, SessionWatchState>,
    watch_id: String,
) -> Result<bool, String> {
    let removed = state
        .watches
        .lock()
        .map_err(|_| "failed to lock session watch state".to_string())?
        .remove(&watch_id)
        .is_some();

    if removed {
        emit_session_watch_event(
            &app,
            SessionWatchEventPayload {
                watch_id,
                runtime_home: String::new(),
                changed_paths: Vec::new(),
                event_tags: vec!["watch_stopped".to_string()],
                timestamp_ms: now_timestamp_ms(),
            },
        );
    }

    Ok(removed)
}

fn stop_existing_watch(state: &SessionWatchState, watch_id: &str) {
    if let Ok(mut watches) = state.watches.lock() {
        watches.remove(watch_id);
    }
}

fn create_watch_plan(runtime_home: &str) -> Result<SessionWatchPlan, String> {
    let runtime_home_path = PathBuf::from(runtime_home);
    if !runtime_home_path.exists() {
        return Err(format!(
            "runtime home does not exist: {}",
            runtime_home_path.display()
        ));
    }

    if !runtime_home_path.is_dir() {
        return Err(format!(
            "runtime home is not a directory: {}",
            runtime_home_path.display()
        ));
    }

    let runtime_home_path = runtime_home_path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize runtime home: {error}"))?;
    let sessions_dir = runtime_home_path.join("sessions");
    let sqlite_path = runtime_home_path.join("state_5.sqlite");
    let sqlite_wal_path = runtime_home_path.join("state_5.sqlite-wal");
    let history_path = runtime_home_path.join("history.jsonl");

    let mut watch_targets = Vec::new();
    push_watch_target(
        &mut watch_targets,
        existing_or_parent(&sqlite_path, &runtime_home_path),
        false,
        sqlite_path.exists(),
        "watch SQLite index updates".to_string(),
    );
    push_watch_target(
        &mut watch_targets,
        existing_or_parent(&sqlite_wal_path, &runtime_home_path),
        false,
        sqlite_wal_path.exists(),
        "watch SQLite WAL updates".to_string(),
    );
    push_watch_target(
        &mut watch_targets,
        existing_or_parent(&history_path, &runtime_home_path),
        false,
        history_path.exists(),
        "watch prompt history updates".to_string(),
    );
    push_watch_target(
        &mut watch_targets,
        existing_or_parent(&sessions_dir, &runtime_home_path),
        true,
        sessions_dir.exists(),
        "watch rollout session trees".to_string(),
    );

    Ok(SessionWatchPlan {
        watch_id: watch_id_for_path(&runtime_home_path),
        runtime_home: runtime_home_path.display().to_string(),
        watch_targets,
    })
}

fn push_watch_target(
    targets: &mut Vec<SessionWatchTarget>,
    path: PathBuf,
    recursive: bool,
    exists: bool,
    reason: String,
) {
    let path_string = path.display().to_string();
    if targets.iter().any(|target| target.path == path_string) {
        return;
    }

    targets.push(SessionWatchTarget {
        path: path_string,
        recursive,
        exists,
        reason,
    });
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

fn watch_id_for_path(runtime_home_path: &Path) -> String {
    format!("codex-session-watch:{}", runtime_home_path.display())
}

fn is_relevant_session_path(path: &Path, runtime_home: &Path) -> bool {
    let Ok(relative_path) = path.strip_prefix(runtime_home) else {
        return false;
    };

    if relative_path == Path::new("state_5.sqlite")
        || relative_path == Path::new("state_5.sqlite-wal")
        || relative_path == Path::new("history.jsonl")
    {
        return true;
    }

    relative_path
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        == Some("sessions")
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
}

fn emit_session_watch_event(app: &AppHandle, payload: SessionWatchEventPayload) {
    let _ = app.emit(SESSION_WATCH_EVENT, payload);
}

fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{create_watch_plan, is_relevant_session_path};
    use std::fs;
    use std::path::Path;

    #[test]
    fn session_artifact_filter_matches_expected_paths() {
        let runtime_home = Path::new("/tmp/codex-home");

        assert!(is_relevant_session_path(
            &runtime_home.join("state_5.sqlite"),
            runtime_home
        ));
        assert!(is_relevant_session_path(
            &runtime_home.join("state_5.sqlite-wal"),
            runtime_home
        ));
        assert!(is_relevant_session_path(
            &runtime_home.join("history.jsonl"),
            runtime_home
        ));
        assert!(is_relevant_session_path(
            &runtime_home.join("sessions/2026/06/29/rollout-abc.jsonl"),
            runtime_home
        ));
        assert!(!is_relevant_session_path(
            &runtime_home.join("sessions/2026/06/29/notes.jsonl"),
            runtime_home
        ));
        assert!(!is_relevant_session_path(
            &runtime_home.join("logs_2.sqlite"),
            runtime_home
        ));
    }

    #[test]
    fn watch_plan_falls_back_to_runtime_home_for_missing_targets() {
        let temp_dir = std::env::temp_dir().join(format!(
            "coding-agent-va-session-watch-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("create temp runtime home");

        let plan =
            create_watch_plan(temp_dir.to_str().expect("utf8 temp dir")).expect("build watch plan");
        let canonical_temp_dir = temp_dir
            .canonicalize()
            .expect("canonicalize temp runtime home");

        assert_eq!(plan.watch_targets.len(), 1);
        assert!(plan
            .watch_targets
            .iter()
            .all(|target| target.path == canonical_temp_dir.display().to_string()));

        fs::remove_dir_all(temp_dir).expect("cleanup temp runtime home");
    }
}
