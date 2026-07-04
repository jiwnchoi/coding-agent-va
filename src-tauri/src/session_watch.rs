use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;
use watchexec::Watchexec;

const SESSION_WATCH_EVENT: &str = "codex-session-watch-event";
const SESSION_TITLE_MAX_CHARS: usize = 120;

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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionSummary {
    pub id: String,
    pub title: String,
    pub rollout_path: String,
    pub cwd: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionList {
    pub runtime_home: String,
    pub sessions: Vec<CodexSessionSummary>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionFileActivity {
    pub read_files: Vec<String>,
    pub edited_files: Vec<String>,
    pub deleted_files: Vec<String>,
}

#[derive(Deserialize)]
struct SessionIndexEntry {
    id: String,
    thread_name: Option<String>,
}

#[derive(Deserialize)]
struct RolloutEnvelope {
    #[serde(rename = "type")]
    entry_type: String,
    payload: serde_json::Value,
}

#[derive(Deserialize)]
struct ExecCommandArguments {
    cmd: String,
    workdir: Option<String>,
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
pub fn list_codex_sessions(runtime_home: Option<String>) -> Result<CodexSessionList, String> {
    let runtime_home_path = resolve_runtime_home(runtime_home)?;
    let session_titles = read_session_titles(&runtime_home_path);
    let sessions_dir = runtime_home_path.join("sessions");

    let mut sessions = WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let file_name = entry.file_name().to_str()?;
            if !(file_name.starts_with("rollout-") && file_name.ends_with(".jsonl")) {
                return None;
            }

            let session_id = extract_session_id(file_name)?;
            let metadata = entry.metadata().ok()?;
            let updated_at_ms = metadata
                .modified()
                .ok()
                .and_then(system_time_to_ms)
                .unwrap_or_default();
            let cwd = read_session_meta_cwd(entry.path());
            let title = read_first_user_prompt_title(entry.path())
                .or_else(|| {
                    session_titles
                        .get(&session_id)
                        .cloned()
                        .map(normalize_title_whitespace)
                })
                .or_else(|| {
                    cwd.as_ref()
                        .and_then(|path| Path::new(path).file_name()?.to_str().map(str::to_string))
                })
                .unwrap_or_else(|| session_id.clone());

            Some(CodexSessionSummary {
                id: session_id,
                title,
                rollout_path: entry.path().display().to_string(),
                cwd,
                updated_at_ms,
            })
        })
        .collect::<Vec<_>>();

    sessions.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));

    Ok(CodexSessionList {
        runtime_home: runtime_home_path.display().to_string(),
        sessions,
    })
}

#[tauri::command]
pub fn get_codex_session_file_activity(
    rollout_path: String,
    cwd: Option<String>,
) -> Result<CodexSessionFileActivity, String> {
    read_codex_session_file_activity(&PathBuf::from(rollout_path), cwd.as_deref())
}

#[tauri::command]
pub async fn start_codex_session_watch(
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

fn resolve_runtime_home(runtime_home: Option<String>) -> Result<PathBuf, String> {
    let candidate = runtime_home
        .map(PathBuf::from)
        .or_else(default_runtime_home)
        .ok_or_else(|| "failed to resolve Codex runtime home".to_string())?;

    if !candidate.exists() {
        return Err(format!(
            "runtime home does not exist: {}",
            candidate.display()
        ));
    }

    if !candidate.is_dir() {
        return Err(format!(
            "runtime home is not a directory: {}",
            candidate.display()
        ));
    }

    candidate
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize runtime home: {error}"))
}

fn default_runtime_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex"))
}

fn read_session_titles(runtime_home: &Path) -> HashMap<String, String> {
    let session_index_path = runtime_home.join("session_index.jsonl");
    let Ok(file) = File::open(session_index_path) else {
        return HashMap::new();
    };

    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<SessionIndexEntry>(&line).ok())
        .filter_map(|entry| entry.thread_name.map(|title| (entry.id, title)))
        .collect()
}

fn extract_session_id(file_name: &str) -> Option<String> {
    file_name
        .strip_prefix("rollout-")?
        .strip_suffix(".jsonl")?
        .rsplit_once('-')
        .map(|(_, session_id)| session_id.to_string())
}

fn read_session_meta_cwd(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let first_line = BufReader::new(file).lines().next()?.ok()?;
    let json = serde_json::from_str::<serde_json::Value>(&first_line).ok()?;

    json.get("payload")?
        .get("cwd")?
        .as_str()
        .map(str::to_string)
}

fn read_first_user_prompt_title(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(envelope) = serde_json::from_str::<RolloutEnvelope>(&line) else {
            continue;
        };
        if envelope.entry_type != "response_item" {
            continue;
        }

        let payload = &envelope.payload;
        if payload.get("type")?.as_str()? != "message" || payload.get("role")?.as_str()? != "user" {
            continue;
        }

        let Some(content) = payload.get("content").and_then(|value| value.as_array()) else {
            continue;
        };
        for item in content {
            if item.get("type").and_then(|value| value.as_str()) != Some("input_text") {
                continue;
            }

            let Some(text) = item.get("text").and_then(|value| value.as_str()) else {
                continue;
            };
            let title = derive_session_title(text);
            if !title.is_empty() && !is_metadata_prompt(&title) {
                return Some(title);
            }
        }
    }

    None
}

fn read_codex_session_file_activity(
    rollout_path: &Path,
    cwd: Option<&str>,
) -> Result<CodexSessionFileActivity, String> {
    let file = File::open(rollout_path)
        .map_err(|error| format!("failed to open rollout {}: {error}", rollout_path.display()))?;
    let workspace_root = cwd.map(PathBuf::from);
    let mut read_files = HashMap::<String, u64>::new();
    let mut edited_files = HashMap::<String, u64>::new();
    let mut deleted_files = HashMap::<String, u64>::new();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(envelope) = serde_json::from_str::<RolloutEnvelope>(&line) else {
            continue;
        };

        let timestamp_ms = envelope
            .payload
            .get("timestamp")
            .and_then(|value| value.as_str())
            .and_then(timestamp_string_to_ms)
            .unwrap_or_default();

        match envelope.entry_type.as_str() {
            "response_item" => collect_read_files(
                &envelope.payload,
                workspace_root.as_deref(),
                timestamp_ms,
                &mut read_files,
            ),
            "event_msg" => collect_written_files(
                &envelope.payload,
                timestamp_ms,
                &mut edited_files,
                &mut deleted_files,
            ),
            _ => {}
        }
    }

    Ok(CodexSessionFileActivity {
        read_files: sort_file_activity(read_files),
        edited_files: sort_file_activity(edited_files),
        deleted_files: sort_file_activity(deleted_files),
    })
}

fn collect_read_files(
    payload: &serde_json::Value,
    workspace_root: Option<&Path>,
    timestamp_ms: u64,
    read_files: &mut HashMap<String, u64>,
) {
    if payload.get("type").and_then(|value| value.as_str()) != Some("function_call") {
        return;
    }

    if payload.get("name").and_then(|value| value.as_str()) != Some("exec_command") {
        return;
    }

    let Some(arguments) = payload.get("arguments").and_then(|value| value.as_str()) else {
        return;
    };
    let Ok(exec_arguments) = serde_json::from_str::<ExecCommandArguments>(arguments) else {
        return;
    };
    let command_root = exec_arguments
        .workdir
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| workspace_root.map(Path::to_path_buf));

    let Some(command_root) = command_root else {
        return;
    };

    for token in shell_like_tokens(&exec_arguments.cmd) {
        let Some(path) = normalize_activity_path(&token, &command_root) else {
            continue;
        };

        read_files
            .entry(path)
            .and_modify(|current| *current = (*current).max(timestamp_ms))
            .or_insert(timestamp_ms);
    }
}

fn collect_written_files(
    payload: &serde_json::Value,
    timestamp_ms: u64,
    edited_files: &mut HashMap<String, u64>,
    deleted_files: &mut HashMap<String, u64>,
) {
    if payload.get("type").and_then(|value| value.as_str()) != Some("patch_apply_end") {
        return;
    }

    let Some(changes) = payload.get("changes").and_then(|value| value.as_object()) else {
        return;
    };

    for (path, change) in changes {
        let Some(change_type) = change.get("type").and_then(|value| value.as_str()) else {
            continue;
        };

        match change_type {
            "delete" => {
                deleted_files
                    .entry(path.clone())
                    .and_modify(|current| *current = (*current).max(timestamp_ms))
                    .or_insert(timestamp_ms);
            }
            "add" | "update" | "move" => {
                edited_files
                    .entry(path.clone())
                    .and_modify(|current| *current = (*current).max(timestamp_ms))
                    .or_insert(timestamp_ms);
            }
            _ => {}
        }
    }
}

fn shell_like_tokens(command: &str) -> Vec<String> {
    command
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ';' | '|' | '&' | ','
                )
        })
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_activity_path(token: &str, workspace_root: &Path) -> Option<String> {
    if token.starts_with('-')
        || token.contains('*')
        || token.contains('$')
        || token.contains("&&")
        || token.contains("||")
        || token == "."
        || token == ".."
    {
        return None;
    }

    let workspace_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let candidate = if token.starts_with('/') {
        PathBuf::from(token)
    } else {
        workspace_root.join(token)
    };

    let normalized = candidate.canonicalize().ok()?;
    if !normalized.is_file() || !normalized.starts_with(workspace_root) {
        return None;
    }

    Some(normalized.display().to_string())
}

fn sort_file_activity(activity: HashMap<String, u64>) -> Vec<String> {
    let mut entries = activity.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    entries.into_iter().map(|(path, _)| path).collect()
}

fn normalize_title_whitespace(text: impl AsRef<str>) -> String {
    strip_image_attachment_markers(text.as_ref())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn derive_session_title(text: &str) -> String {
    let sanitized = strip_image_attachment_markers(text);
    let non_empty_lines = sanitized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let normalized_full_text = normalize_title_whitespace(&sanitized);
    if non_empty_lines.len() < 3 && normalized_full_text.chars().count() <= SESSION_TITLE_MAX_CHARS
    {
        return normalized_full_text;
    }

    let first_non_empty_line = non_empty_lines
        .first()
        .copied()
        .unwrap_or_default()
        .to_string();
    let normalized = normalize_title_whitespace(first_non_empty_line);
    truncate_title(&normalized, SESSION_TITLE_MAX_CHARS)
}

fn truncate_title(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let truncated = text
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        .trim_end()
        .to_string();

    format!("{truncated}…")
}

fn strip_image_attachment_markers(text: &str) -> String {
    let mut sanitized = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(marker_start) = remaining.find("<image ") {
        sanitized.push_str(&remaining[..marker_start]);

        let after_marker_start = &remaining[marker_start..];
        let Some(marker_end) = after_marker_start.find('>') else {
            sanitized.push_str(after_marker_start);
            return sanitized;
        };

        remaining = &after_marker_start[marker_end + 1..];
    }

    sanitized.push_str(remaining);
    sanitized
}

fn is_metadata_prompt(text: &str) -> bool {
    if text.starts_with("# AGENTS.md instructions") {
        return true;
    }

    let first_token = text.split_whitespace().next().unwrap_or_default();
    first_token.starts_with('<') && first_token.ends_with('>')
}

fn create_watch_plan(runtime_home: &str) -> Result<SessionWatchPlan, String> {
    let runtime_home_path = resolve_runtime_home(Some(runtime_home.to_string()))?;
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

fn system_time_to_ms(system_time: SystemTime) -> Option<u64> {
    system_time
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn timestamp_string_to_ms(timestamp: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.timestamp_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::{
        create_watch_plan, derive_session_title, is_metadata_prompt, is_relevant_session_path,
        normalize_title_whitespace, read_codex_session_file_activity, read_first_user_prompt_title,
        SESSION_TITLE_MAX_CHARS,
    };
    use std::fs::{self, File};
    use std::io::Write;
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

    #[test]
    fn extracts_first_meaningful_user_prompt_for_session_title() {
        let temp_dir = std::env::temp_dir().join(format!(
            "coding-agent-va-session-title-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("create temp directory");

        let rollout_path = temp_dir.join("rollout-test.jsonl");
        let mut file = File::create(&rollout_path).expect("create rollout file");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "# AGENTS.md instructions for /tmp/repo" }]
                }
            })
        )
        .expect("write agents row");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "<environment_context> cwd=/tmp </environment_context>" }]
                }
            })
        )
        .expect("write env row");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "session 상단에 검색 기능 추가해주고,\n\n첫 유저 프롬프트를 보여줘" }]
                }
            })
        )
        .expect("write real prompt row");

        assert_eq!(
            read_first_user_prompt_title(&rollout_path),
            Some("session 상단에 검색 기능 추가해주고, 첫 유저 프롬프트를 보여줘".to_string())
        );

        fs::remove_dir_all(temp_dir).expect("cleanup temp directory");
    }

    #[test]
    fn metadata_prompt_detection_matches_known_wrappers() {
        assert!(is_metadata_prompt("# AGENTS.md instructions for /tmp/repo"));
        assert!(is_metadata_prompt(
            "<environment_context> cwd=/tmp </environment_context>"
        ));
        assert!(is_metadata_prompt(
            "<turn_aborted> interrupted </turn_aborted>"
        ));
        assert!(!is_metadata_prompt("실제 사용자 요청입니다"));
    }

    #[test]
    fn title_normalization_collapses_whitespace() {
        assert_eq!(
            normalize_title_whitespace("first line\n\n second\tline"),
            "first line second line".to_string()
        );
    }

    #[test]
    fn title_normalization_removes_image_attachment_markers() {
        assert_eq!(
            normalize_title_whitespace(
                r#"<image name=[Image #1] path="/var/folders/tmp/codex-clipboard.png">
세션 제목 파싱할 때 이런 이미지 제거하게 해줘"#,
            ),
            "세션 제목 파싱할 때 이런 이미지 제거하게 해줘".to_string()
        );
    }

    #[test]
    fn session_title_uses_first_non_empty_line() {
        assert_eq!(
            derive_session_title(
                "\n\n제목 한 줄만 보여줘\n\n아래는 아주 긴 기술 문서 본문입니다.\n두 번째 줄도 있습니다."
            ),
            "제목 한 줄만 보여줘".to_string()
        );
    }

    #[test]
    fn session_title_truncates_long_single_line_input() {
        let long_title = "a".repeat(SESSION_TITLE_MAX_CHARS + 20);
        let derived = derive_session_title(&long_title);

        assert_eq!(derived.chars().count(), SESSION_TITLE_MAX_CHARS);
        assert!(derived.ends_with('…'));
    }

    #[test]
    fn skips_image_only_prompt_for_session_title() {
        let temp_dir = std::env::temp_dir().join(format!(
            "coding-agent-va-image-title-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("create temp directory");

        let rollout_path = temp_dir.join("rollout-test.jsonl");
        let mut file = File::create(&rollout_path).expect("create rollout file");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": r#"<image name=[Image #1] path="/var/folders/tmp/codex-clipboard.png">"#
                    }]
                }
            })
        )
        .expect("write image row");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "실제 사용자 요청입니다" }]
                }
            })
        )
        .expect("write real prompt row");

        assert_eq!(
            read_first_user_prompt_title(&rollout_path),
            Some("실제 사용자 요청입니다".to_string())
        );

        fs::remove_dir_all(temp_dir).expect("cleanup temp directory");
    }

    #[test]
    fn extracts_read_edited_and_deleted_files_from_rollout() {
        let temp_dir = std::env::temp_dir().join(format!(
            "coding-agent-va-file-activity-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("src")).expect("create src directory");

        let read_path = temp_dir.join("src/App.tsx");
        let edited_path = temp_dir.join("src/index.css");
        let deleted_path = temp_dir.join("src/old.ts");
        fs::write(&read_path, "read").expect("write read file");
        fs::write(&edited_path, "edit").expect("write edited file");

        let rollout_path = temp_dir.join("rollout-test.jsonl");
        let mut file = File::create(&rollout_path).expect("create rollout file");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "timestamp": "2026-07-04T06:12:00.000Z",
                    "type": "function_call",
                    "name": "exec_command",
                    "arguments": serde_json::json!({
                        "cmd": "sed -n '1,220p' src/App.tsx",
                        "workdir": temp_dir.display().to_string()
                    }).to_string()
                }
            })
        )
        .expect("write read activity");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "timestamp": "2026-07-04T06:12:10.000Z",
                    "type": "patch_apply_end",
                    "changes": {
                        edited_path.display().to_string(): { "type": "update" },
                        deleted_path.display().to_string(): { "type": "delete" }
                    }
                }
            })
        )
        .expect("write patch activity");

        let activity = read_codex_session_file_activity(
            &rollout_path,
            Some(temp_dir.to_str().expect("utf8 temp dir")),
        )
        .expect("read file activity");

        assert_eq!(
            activity.read_files,
            vec![read_path
                .canonicalize()
                .expect("canonical read path")
                .display()
                .to_string()]
        );
        assert_eq!(
            activity.edited_files,
            vec![edited_path.display().to_string()]
        );
        assert_eq!(
            activity.deleted_files,
            vec![deleted_path.display().to_string()]
        );

        fs::remove_dir_all(temp_dir).expect("cleanup temp directory");
    }
}
