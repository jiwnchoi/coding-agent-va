use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;

use crate::indexer::workspace_dependencies::find_impacted_files;
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
    pub git_index_paths: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchRegistration {
    pub watch_id: String,
    pub runtime_home: String,
    pub watch_targets: Vec<SessionWatchTarget>,
    pub git_index_paths: Vec<String>,
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
    pub impacted_files: Vec<String>,
    pub deleted_files: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionFileDiff {
    pub file_path: String,
    pub display_path: String,
    pub original_content: String,
    pub modified_content: String,
    pub diff_base_label: String,
    pub diff_target_label: String,
    pub file_missing: bool,
    pub is_tracked: bool,
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
pub fn get_codex_session_file_diff(
    file_path: String,
    cwd: Option<String>,
) -> Result<CodexSessionFileDiff, String> {
    read_codex_session_file_diff(&PathBuf::from(file_path), cwd.as_deref())
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
    let git_index_paths = plan
        .git_index_paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
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
    let git_index_paths_for_task = git_index_paths.clone();
    let wx = Watchexec::new(move |action| {
        let changed_paths = action
            .events
            .iter()
            .flat_map(|event| event.paths().map(|(path, _)| path.to_path_buf()))
            .map(|path| normalize_watch_event_path(&path))
            .filter(|path| {
                is_relevant_watch_path(path, &runtime_home_for_task, &git_index_paths_for_task)
            })
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

    wx.config.throttle(Duration::from_millis(500));
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
        git_index_paths: plan.git_index_paths,
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

    filter_written_files_by_git_status(cwd, &mut edited_files, &mut deleted_files);
    remove_edited_files_from_read_files(cwd, &mut read_files, &edited_files);
    let impacted_files = resolve_impacted_files(cwd, &edited_files, &deleted_files)?;

    Ok(CodexSessionFileActivity {
        read_files: sort_file_activity(read_files),
        edited_files: sort_file_activity(edited_files),
        impacted_files,
        deleted_files: sort_file_activity(deleted_files),
    })
}

fn resolve_impacted_files(
    cwd: Option<&str>,
    edited_files: &HashMap<String, u64>,
    deleted_files: &HashMap<String, u64>,
) -> Result<Vec<String>, String> {
    let Some(workspace_root) = cwd.map(PathBuf::from) else {
        return Ok(Vec::new());
    };

    let changed_files = edited_files
        .keys()
        .chain(deleted_files.keys())
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    find_impacted_files(&workspace_root, &changed_files)
}

fn read_codex_session_file_diff(
    file_path: &Path,
    cwd: Option<&str>,
) -> Result<CodexSessionFileDiff, String> {
    let absolute_file_path = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        let workspace_root = cwd
            .map(PathBuf::from)
            .ok_or_else(|| format!("relative file path requires cwd: {}", file_path.display()))?;
        workspace_root.join(file_path)
    };
    let display_path = cwd
        .and_then(|workspace_root| {
            absolute_file_path
                .strip_prefix(workspace_root)
                .ok()
                .map(|relative_path| relative_path.display().to_string())
        })
        .unwrap_or_else(|| absolute_file_path.display().to_string());
    let modified_content = fs::read_to_string(&absolute_file_path).unwrap_or_default();
    let file_missing = !absolute_file_path.is_file();
    let git_snapshot = read_git_head_snapshot(&absolute_file_path, cwd);
    let (original_content, is_tracked) = match git_snapshot {
        Some(content) => (content, true),
        None if file_missing => (String::new(), false),
        None => (modified_content.clone(), false),
    };

    Ok(CodexSessionFileDiff {
        file_path: absolute_file_path.display().to_string(),
        display_path,
        original_content,
        modified_content,
        diff_base_label: "HEAD".to_string(),
        diff_target_label: if file_missing {
            "Deleted".to_string()
        } else if is_tracked {
            "Working tree".to_string()
        } else {
            "Workspace".to_string()
        },
        file_missing,
        is_tracked,
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

fn remove_edited_files_from_read_files(
    cwd: Option<&str>,
    read_files: &mut HashMap<String, u64>,
    edited_files: &HashMap<String, u64>,
) {
    let edited_paths = edited_files
        .keys()
        .filter_map(|path| normalize_written_activity_path(path, cwd))
        .collect::<HashSet<_>>();

    read_files.retain(|path, _| {
        normalize_written_activity_path(path, cwd)
            .map(|normalized_path| !edited_paths.contains(&normalized_path))
            .unwrap_or(true)
    });
}

fn filter_written_files_by_git_status(
    cwd: Option<&str>,
    edited_files: &mut HashMap<String, u64>,
    deleted_files: &mut HashMap<String, u64>,
) {
    let Some(git_status) = cwd.and_then(read_git_worktree_status) else {
        return;
    };

    edited_files.retain(|path, _| {
        normalize_written_activity_path(path, cwd)
            .is_some_and(|normalized_path| git_status.edited_files.contains(&normalized_path))
    });
    deleted_files.retain(|path, _| {
        normalize_written_activity_path(path, cwd)
            .is_some_and(|normalized_path| git_status.deleted_files.contains(&normalized_path))
    });
}

#[derive(Default)]
struct GitWorktreeStatus {
    edited_files: HashSet<String>,
    deleted_files: HashSet<String>,
}

fn read_git_worktree_status(cwd: &str) -> Option<GitWorktreeStatus> {
    let repo_root = resolve_git_repo_root_from_path(Path::new(cwd))?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let mut status = GitWorktreeStatus::default();
    let mut entries = output.stdout.split(|byte| *byte == 0);
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }

        let index_status = entry[0] as char;
        let worktree_status = entry[1] as char;
        let path = String::from_utf8_lossy(&entry[3..]).to_string();
        if path.is_empty() {
            continue;
        }

        if matches!(index_status, 'R' | 'C') {
            let _ = entries.next();
        }

        let absolute_path = normalize_absolute_activity_path(&repo_root.join(path));
        if index_status == 'D' || worktree_status == 'D' {
            status.deleted_files.insert(absolute_path);
        } else {
            status.edited_files.insert(absolute_path);
        }
    }

    Some(status)
}

fn normalize_written_activity_path(path: &str, cwd: Option<&str>) -> Option<String> {
    let path = Path::new(path);
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let cwd = PathBuf::from(cwd?);
        cwd.canonicalize().unwrap_or(cwd).join(path)
    };

    Some(normalize_absolute_activity_path(&absolute_path))
}

fn normalize_absolute_activity_path(path: &Path) -> String {
    if let Ok(canonical_path) = path.canonicalize() {
        return canonical_path.display().to_string();
    }

    path.parent()
        .and_then(|parent| {
            let canonical_parent = parent.canonicalize().ok()?;
            let file_name = path.file_name()?;
            Some(canonical_parent.join(file_name).display().to_string())
        })
        .unwrap_or_else(|| path.display().to_string())
}

fn read_git_head_snapshot(file_path: &Path, cwd: Option<&str>) -> Option<String> {
    let repo_root = resolve_git_repo_root(file_path, cwd)?;
    let relative_path = file_path.strip_prefix(&repo_root).ok()?;
    let git_object_path = relative_path.to_string_lossy().replace('\\', "/");
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .arg("show")
        .arg(format!("HEAD:{git_object_path}"))
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn resolve_git_repo_root(file_path: &Path, cwd: Option<&str>) -> Option<PathBuf> {
    let search_root = if file_path.is_file() {
        file_path.parent().map(Path::to_path_buf)
    } else {
        Some(file_path.to_path_buf())
    }
    .or_else(|| cwd.map(PathBuf::from))?;
    resolve_git_repo_root_from_path(&search_root)
}

fn resolve_git_repo_root_from_path(search_root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(search_root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let repo_root = String::from_utf8(output.stdout).ok()?;
    let trimmed = repo_root.trim();
    if trimmed.is_empty() {
        return None;
    }

    let repo_root = PathBuf::from(trimmed);
    Some(repo_root.canonicalize().unwrap_or(repo_root))
}

fn resolve_git_index_path(repo_root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--git-path")
        .arg("index")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let index_path = String::from_utf8(output.stdout).ok()?;
    let trimmed = index_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = PathBuf::from(trimmed);
    let index_path = if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    };
    Some(PathBuf::from(normalize_absolute_activity_path(&index_path)))
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
    let git_index_paths = collect_session_git_index_paths(&sessions_dir);

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
    for git_index_path in &git_index_paths {
        push_watch_target(
            &mut watch_targets,
            existing_or_parent(git_index_path, &runtime_home_path),
            false,
            git_index_path.exists(),
            "watch git index updates".to_string(),
        );
    }

    Ok(SessionWatchPlan {
        watch_id: watch_id_for_path(&runtime_home_path),
        runtime_home: runtime_home_path.display().to_string(),
        watch_targets,
        git_index_paths: git_index_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
    })
}

fn collect_session_git_index_paths(sessions_dir: &Path) -> Vec<PathBuf> {
    if !sessions_dir.exists() {
        return Vec::new();
    }

    let mut repo_roots = BTreeSet::<PathBuf>::new();
    for entry in WalkDir::new(sessions_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let file_name = entry.file_name().to_string_lossy();
        if !(file_name.starts_with("rollout-") && file_name.ends_with(".jsonl")) {
            continue;
        }

        let Some(cwd) = read_session_meta_cwd(entry.path()) else {
            continue;
        };
        let Some(repo_root) = resolve_git_repo_root_from_path(Path::new(&cwd)) else {
            continue;
        };

        repo_roots.insert(repo_root);
    }

    repo_roots
        .into_iter()
        .map(|repo_root| {
            resolve_git_index_path(&repo_root).unwrap_or_else(|| repo_root.join(".git/index"))
        })
        .collect()
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

fn is_relevant_watch_path(path: &Path, runtime_home: &Path, git_index_paths: &[PathBuf]) -> bool {
    is_relevant_session_path(path, runtime_home)
        || git_index_paths.iter().any(|index_path| path == index_path)
}

fn normalize_watch_event_path(path: &Path) -> PathBuf {
    PathBuf::from(normalize_absolute_activity_path(path))
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
        resolve_git_index_path, SESSION_TITLE_MAX_CHARS,
    };
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use std::process::Command;

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
    fn watch_plan_includes_git_targets_for_session_cwds() {
        let temp_dir = std::env::temp_dir().join(format!(
            "coding-agent-va-session-git-watch-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);

        let runtime_home = temp_dir.join("codex-home");
        let sessions_dir = runtime_home.join("sessions/2026/07/04");
        let repo_dir = temp_dir.join("repo");
        fs::create_dir_all(&sessions_dir).expect("create sessions directory");
        fs::create_dir_all(&repo_dir).expect("create repo directory");

        run_git(&repo_dir, &["init"]);
        fs::write(repo_dir.join("tracked.txt"), "initial").expect("write repo file");
        run_git(&repo_dir, &["config", "user.email", "codex@example.com"]);
        run_git(&repo_dir, &["config", "user.name", "Codex"]);
        run_git(&repo_dir, &["add", "."]);
        run_git(&repo_dir, &["commit", "-m", "initial"]);

        let rollout_path = sessions_dir.join("rollout-2026-07-04-test.jsonl");
        let mut file = File::create(&rollout_path).expect("create rollout file");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "session_meta",
                "payload": {
                    "cwd": repo_dir.display().to_string()
                }
            })
        )
        .expect("write session metadata");

        let plan = create_watch_plan(runtime_home.to_str().expect("utf8 runtime home"))
            .expect("build watch plan");
        let repo_root = repo_dir.canonicalize().expect("canonical repo root");
        let git_index_path = resolve_git_index_path(&repo_root).expect("resolve git index path");

        assert!(plan
            .git_index_paths
            .contains(&git_index_path.display().to_string()));
        assert!(!plan
            .watch_targets
            .iter()
            .any(|target| target.path == repo_root.display().to_string()));
        assert!(plan.watch_targets.iter().any(|target| {
            target.path == git_index_path.display().to_string()
                && target.reason == "watch git index updates"
        }));

        fs::remove_dir_all(temp_dir).expect("cleanup temp directory");
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
                        "cmd": "sed -n '1,220p' src/App.tsx src/index.css",
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

    #[test]
    fn filters_edited_and_deleted_files_to_git_status_intersection() {
        let temp_dir = std::env::temp_dir().join(format!(
            "coding-agent-va-git-activity-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("src")).expect("create src directory");

        let edited_path = temp_dir.join("src/changed.ts");
        let stale_edited_path = temp_dir.join("src/stale.ts");
        let deleted_path = temp_dir.join("src/deleted.ts");
        let stale_deleted_path = temp_dir.join("src/not-deleted.ts");

        for path in [
            &edited_path,
            &stale_edited_path,
            &deleted_path,
            &stale_deleted_path,
        ] {
            fs::write(path, "initial").expect("write initial file");
        }

        run_git(&temp_dir, &["init"]);
        run_git(&temp_dir, &["config", "user.email", "codex@example.com"]);
        run_git(&temp_dir, &["config", "user.name", "Codex"]);
        run_git(&temp_dir, &["add", "."]);
        run_git(&temp_dir, &["commit", "-m", "initial"]);

        fs::write(&edited_path, "changed").expect("modify edited file");
        fs::remove_file(&deleted_path).expect("delete tracked file");

        let rollout_path = temp_dir.join("rollout-test.jsonl");
        let mut file = File::create(&rollout_path).expect("create rollout file");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "timestamp": "2026-07-04T06:12:10.000Z",
                    "type": "patch_apply_end",
                    "changes": {
                        "src/changed.ts": { "type": "update" },
                        "src/stale.ts": { "type": "update" },
                        "src/deleted.ts": { "type": "delete" },
                        "src/not-deleted.ts": { "type": "delete" }
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

        assert_eq!(activity.edited_files, vec!["src/changed.ts".to_string()]);
        assert_eq!(activity.deleted_files, vec!["src/deleted.ts".to_string()]);

        fs::remove_dir_all(temp_dir).expect("cleanup temp directory");
    }

    fn run_git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .expect("run git command");

        assert!(
            output.status.success(),
            "git {:?} failed: {}{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
