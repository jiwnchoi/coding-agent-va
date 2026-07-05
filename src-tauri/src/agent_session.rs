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
use ts_rs::TS;
use walkdir::WalkDir;
use watchexec::Watchexec;

use crate::indexer::workspace_dependencies::{
    find_impacted_file_relations, ImpactedFileRelation as WorkspaceImpactedFileRelation,
};

const AGENT_SESSION_WATCH_EVENT: &str = "agent-session-watch-event";
const SESSION_TITLE_MAX_CHARS: usize = 120;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum AgentSessionProvider {
    Codex,
    Claude,
    Pi,
}

impl AgentSessionProvider {
    fn protocol(self) -> Box<dyn AgentSessionProtocol> {
        match self {
            Self::Codex => Box::new(CodexSessionProtocol),
            Self::Claude => Box::new(ClaudeSessionProtocol),
            Self::Pi => Box::new(PiSessionProtocol),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::Pi => "Pi Agent",
        }
    }

    fn all() -> [Self; 3] {
        [Self::Codex, Self::Claude, Self::Pi]
    }
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeSource {
    pub provider: AgentSessionProvider,
    pub label: String,
    pub runtime_home: String,
    pub available: bool,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionSummary {
    pub id: String,
    pub provider: AgentSessionProvider,
    pub provider_session_id: String,
    pub provider_label: String,
    pub title: String,
    pub transcript_path: String,
    pub cwd: Option<String>,
    pub runtime_home: String,
    pub updated_at_ms: u64,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionList {
    pub sources: Vec<AgentRuntimeSource>,
    pub sessions: Vec<AgentSessionSummary>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchTarget {
    pub path: String,
    pub recursive: bool,
    pub exists: bool,
    pub reason: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchPlan {
    pub watch_id: String,
    pub provider: AgentSessionProvider,
    pub runtime_home: String,
    pub watch_targets: Vec<SessionWatchTarget>,
    pub git_index_paths: Vec<String>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchRegistration {
    pub watch_id: String,
    pub provider: AgentSessionProvider,
    pub runtime_home: String,
    pub watch_targets: Vec<SessionWatchTarget>,
    pub git_index_paths: Vec<String>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchEventPayload {
    pub watch_id: String,
    pub provider: AgentSessionProvider,
    pub runtime_home: String,
    pub changed_paths: Vec<String>,
    pub event_tags: Vec<String>,
    pub timestamp_ms: u64,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionFileActivity {
    pub read_files: Vec<String>,
    pub edited_files: Vec<String>,
    pub impacted_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub impacted_relations: Vec<AgentSessionImpactedFileRelation>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionImpactedFileRelation {
    pub changed_file: String,
    pub impacted_file: String,
    pub import_specifier: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionFileDiff {
    pub file_path: String,
    pub display_path: String,
    pub original_content: String,
    pub modified_content: String,
    pub diff_base_label: String,
    pub diff_target_label: String,
    pub file_missing: bool,
    pub is_tracked: bool,
}

#[derive(Default)]
pub struct AgentSessionWatchState {
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

trait AgentSessionProtocol: Send + Sync {
    fn provider(&self) -> AgentSessionProvider;
    fn default_runtime_home(&self) -> Option<PathBuf>;
    fn list_sessions(&self, runtime_home: &Path) -> Vec<AgentSessionSummary>;
    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)>;
    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool;
    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String>;

    fn create_watch_plan(&self, runtime_home: &Path) -> Result<SessionWatchPlan, String> {
        let runtime_home = resolve_existing_dir(runtime_home)?;
        let sessions = self.list_sessions(&runtime_home);
        let git_index_paths = collect_git_index_paths_from_sessions(&sessions);
        let mut watch_targets = Vec::new();

        for (target_path, recursive, reason) in self.watch_roots(&runtime_home) {
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

        Ok(SessionWatchPlan {
            watch_id: watch_id_for_provider_path(self.provider(), &runtime_home),
            provider: self.provider(),
            runtime_home: runtime_home.display().to_string(),
            watch_targets,
            git_index_paths: git_index_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
        })
    }

    fn read_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<AgentSessionFileActivity, String> {
        let mut activity = self.collect_file_activity(transcript_path, cwd)?;

        filter_written_files_by_git_status(
            cwd,
            &mut activity.edited_files,
            &mut activity.deleted_files,
        );
        remove_edited_files_from_read_files(cwd, &mut activity.read_files, &activity.edited_files);
        let impacted_relations =
            resolve_impacted_file_relations(cwd, &activity.edited_files, &activity.deleted_files)?;
        let impacted_files = impacted_relations
            .iter()
            .map(|relation| relation.impacted_file.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        Ok(AgentSessionFileActivity {
            read_files: sort_file_activity(activity.read_files),
            edited_files: sort_file_activity(activity.edited_files),
            impacted_files,
            deleted_files: sort_file_activity(activity.deleted_files),
            impacted_relations,
        })
    }
}

struct CodexSessionProtocol;
struct ClaudeSessionProtocol;
struct PiSessionProtocol;

#[derive(Default)]
struct ActivityAccumulator {
    read_files: HashMap<String, u64>,
    edited_files: HashMap<String, u64>,
    deleted_files: HashMap<String, u64>,
}

#[derive(Deserialize)]
struct CodexSessionIndexEntry {
    id: String,
    thread_name: Option<String>,
}

#[derive(Deserialize)]
struct CodexRolloutEnvelope {
    #[serde(rename = "type")]
    entry_type: String,
    payload: serde_json::Value,
}

#[derive(Deserialize)]
struct CodexExecCommandArguments {
    cmd: String,
    workdir: Option<String>,
}

impl AgentSessionProtocol for CodexSessionProtocol {
    fn provider(&self) -> AgentSessionProvider {
        AgentSessionProvider::Codex
    }

    fn default_runtime_home(&self) -> Option<PathBuf> {
        home_dir().map(|home| home.join(".codex"))
    }

    fn list_sessions(&self, runtime_home: &Path) -> Vec<AgentSessionSummary> {
        let session_titles = read_codex_session_titles(runtime_home);
        let sessions_dir = runtime_home.join("sessions");

        list_jsonl_files(&sessions_dir)
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
            })
            .filter_map(|path| {
                let file_name = path.file_name()?.to_str()?;
                let provider_session_id = extract_codex_session_id(file_name)?;
                let updated_at_ms = file_updated_at_ms(&path);
                let cwd = read_codex_session_meta_cwd(&path);
                let title = read_codex_first_user_prompt_title(&path)
                    .or_else(|| {
                        session_titles
                            .get(&provider_session_id)
                            .cloned()
                            .map(normalize_title_whitespace)
                    })
                    .or_else(|| cwd.as_deref().and_then(directory_title))
                    .unwrap_or_else(|| provider_session_id.clone());

                Some(build_session_summary(
                    self.provider(),
                    provider_session_id,
                    title,
                    path,
                    cwd,
                    runtime_home,
                    updated_at_ms,
                ))
            })
            .collect()
    }

    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)> {
        vec![
            (
                runtime_home.join("state_5.sqlite"),
                false,
                "watch SQLite index updates".to_string(),
            ),
            (
                runtime_home.join("state_5.sqlite-wal"),
                false,
                "watch SQLite WAL updates".to_string(),
            ),
            (
                runtime_home.join("history.jsonl"),
                false,
                "watch prompt history updates".to_string(),
            ),
            (
                runtime_home.join("sessions"),
                true,
                "watch rollout session trees".to_string(),
            ),
        ]
    }

    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool {
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
            && is_jsonl_file_name(path, Some("rollout-"))
    }

    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String> {
        let file = File::open(transcript_path).map_err(|error| {
            format!(
                "failed to open Codex rollout {}: {error}",
                transcript_path.display()
            )
        })?;
        let workspace_root = cwd.map(PathBuf::from);
        let mut activity = ActivityAccumulator::default();

        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(envelope) = serde_json::from_str::<CodexRolloutEnvelope>(&line) else {
                continue;
            };
            let timestamp_ms = payload_timestamp_ms(&envelope.payload);

            match envelope.entry_type.as_str() {
                "response_item" => collect_codex_read_files(
                    &envelope.payload,
                    workspace_root.as_deref(),
                    timestamp_ms,
                    &mut activity.read_files,
                ),
                "event_msg" => collect_codex_written_files(
                    &envelope.payload,
                    timestamp_ms,
                    &mut activity.edited_files,
                    &mut activity.deleted_files,
                ),
                _ => {}
            }
        }

        Ok(activity)
    }
}

impl AgentSessionProtocol for PiSessionProtocol {
    fn provider(&self) -> AgentSessionProvider {
        AgentSessionProvider::Pi
    }

    fn default_runtime_home(&self) -> Option<PathBuf> {
        home_dir().map(|home| home.join(".pi").join("agent"))
    }

    fn list_sessions(&self, runtime_home: &Path) -> Vec<AgentSessionSummary> {
        list_jsonl_files(&runtime_home.join("sessions"))
            .into_iter()
            .filter_map(|path| {
                let header = read_first_json_line(&path)?;
                if json_str(&header, &["type"]) != Some("session") {
                    return None;
                }

                let provider_session_id = json_str(&header, &["id"])
                    .map(str::to_string)
                    .or_else(|| file_stem_string(&path))?;
                let cwd = json_str(&header, &["cwd"]).map(str::to_string);
                let updated_at_ms = file_updated_at_ms(&path);
                let title = read_pi_session_name(&path)
                    .or_else(|| read_pi_first_user_prompt_title(&path))
                    .or_else(|| cwd.as_deref().and_then(directory_title))
                    .unwrap_or_else(|| provider_session_id.clone());

                Some(build_session_summary(
                    self.provider(),
                    provider_session_id,
                    title,
                    path,
                    cwd,
                    runtime_home,
                    updated_at_ms,
                ))
            })
            .collect()
    }

    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)> {
        vec![(
            runtime_home.join("sessions"),
            true,
            "watch Pi session transcripts".to_string(),
        )]
    }

    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool {
        path.strip_prefix(runtime_home)
            .ok()
            .and_then(|relative_path| relative_path.components().next())
            .and_then(|component| component.as_os_str().to_str())
            == Some("sessions")
            && is_jsonl_file_name(path, None)
    }

    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String> {
        read_tool_call_file_activity(transcript_path, cwd, ToolSchema::Pi)
    }
}

impl AgentSessionProtocol for ClaudeSessionProtocol {
    fn provider(&self) -> AgentSessionProvider {
        AgentSessionProvider::Claude
    }

    fn default_runtime_home(&self) -> Option<PathBuf> {
        home_dir().map(|home| home.join(".claude"))
    }

    fn list_sessions(&self, runtime_home: &Path) -> Vec<AgentSessionSummary> {
        list_jsonl_files(&runtime_home.join("projects"))
            .into_iter()
            .filter(|path| {
                !path
                    .components()
                    .any(|component| component.as_os_str() == "subagents")
            })
            .filter_map(|path| {
                let metadata = read_claude_session_metadata(&path)?;
                let provider_session_id =
                    metadata.session_id.or_else(|| file_stem_string(&path))?;
                let updated_at_ms = file_updated_at_ms(&path);
                let title = read_claude_ai_title(&path)
                    .or_else(|| read_claude_first_user_prompt_title(&path))
                    .or_else(|| metadata.cwd.as_deref().and_then(directory_title))
                    .unwrap_or_else(|| provider_session_id.clone());

                Some(build_session_summary(
                    self.provider(),
                    provider_session_id,
                    title,
                    path,
                    metadata.cwd,
                    runtime_home,
                    updated_at_ms,
                ))
            })
            .collect()
    }

    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)> {
        vec![
            (
                runtime_home.join("history.jsonl"),
                false,
                "watch Claude prompt history updates".to_string(),
            ),
            (
                runtime_home.join("projects"),
                true,
                "watch Claude project transcripts".to_string(),
            ),
        ]
    }

    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool {
        let Ok(relative_path) = path.strip_prefix(runtime_home) else {
            return false;
        };

        if relative_path == Path::new("history.jsonl") {
            return true;
        }

        relative_path
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
            == Some("projects")
            && is_jsonl_file_name(path, None)
    }

    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String> {
        read_tool_call_file_activity(transcript_path, cwd, ToolSchema::Claude)
    }
}

#[tauri::command]
pub fn list_agent_sessions() -> Result<AgentSessionList, String> {
    let sources = default_runtime_sources();
    let mut sessions = Vec::new();

    for source in &sources {
        if !source.available {
            continue;
        }

        let protocol = source.provider.protocol();
        sessions.extend(protocol.list_sessions(&PathBuf::from(&source.runtime_home)));
    }

    sessions.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));

    Ok(AgentSessionList { sources, sessions })
}

#[tauri::command]
pub fn plan_agent_session_watch(
    provider: AgentSessionProvider,
    runtime_home: String,
) -> Result<SessionWatchPlan, String> {
    provider
        .protocol()
        .create_watch_plan(&PathBuf::from(runtime_home))
}

#[tauri::command]
pub fn get_agent_session_file_activity(
    provider: AgentSessionProvider,
    transcript_path: String,
    cwd: Option<String>,
) -> Result<AgentSessionFileActivity, String> {
    provider
        .protocol()
        .read_file_activity(&PathBuf::from(transcript_path), cwd.as_deref())
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
    let plan = protocol.create_watch_plan(&PathBuf::from(runtime_home))?;
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
            provider,
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
        emit_session_watch_event(
            &app,
            SessionWatchEventPayload {
                watch_id,
                provider: AgentSessionProvider::Codex,
                runtime_home: String::new(),
                changed_paths: Vec::new(),
                event_tags: vec!["watch_stopped".to_string()],
                timestamp_ms: now_timestamp_ms(),
            },
        );
    }

    Ok(removed)
}

pub fn manage_agent_session_watch_state(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.manage(AgentSessionWatchState::default())
}

fn default_runtime_sources() -> Vec<AgentRuntimeSource> {
    AgentSessionProvider::all()
        .into_iter()
        .filter_map(|provider| {
            let protocol = provider.protocol();
            let runtime_home = protocol.default_runtime_home()?;
            Some(AgentRuntimeSource {
                provider,
                label: provider.label().to_string(),
                runtime_home: runtime_home.display().to_string(),
                available: runtime_home.is_dir(),
            })
        })
        .collect()
}

fn build_session_summary(
    provider: AgentSessionProvider,
    provider_session_id: String,
    title: String,
    transcript_path: PathBuf,
    cwd: Option<String>,
    runtime_home: &Path,
    updated_at_ms: u64,
) -> AgentSessionSummary {
    AgentSessionSummary {
        id: format!("{}:{}", provider_key(provider), provider_session_id),
        provider,
        provider_session_id,
        provider_label: provider.label().to_string(),
        title,
        transcript_path: transcript_path.display().to_string(),
        cwd,
        runtime_home: runtime_home.display().to_string(),
        updated_at_ms,
    }
}

fn provider_key(provider: AgentSessionProvider) -> &'static str {
    match provider {
        AgentSessionProvider::Codex => "codex",
        AgentSessionProvider::Claude => "claude",
        AgentSessionProvider::Pi => "pi",
    }
}

fn list_jsonl_files(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }

    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .filter(|path| is_jsonl_file_name(path, None))
        .collect()
}

fn is_jsonl_file_name(path: &Path, prefix: Option<&str>) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            prefix.is_none_or(|prefix| name.starts_with(prefix)) && name.ends_with(".jsonl")
        })
}

fn file_updated_at_ms(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_to_ms)
        .unwrap_or_default()
}

fn read_first_json_line(path: &Path) -> Option<serde_json::Value> {
    let file = File::open(path).ok()?;
    let first_line = BufReader::new(file).lines().next()?.ok()?;
    serde_json::from_str(&first_line).ok()
}

fn file_stem_string(path: &Path) -> Option<String> {
    path.file_stem()?.to_str().map(str::to_string)
}

fn json_str<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str()
}

fn directory_title(path: &str) -> Option<String> {
    Path::new(path).file_name()?.to_str().map(str::to_string)
}

fn read_codex_session_titles(runtime_home: &Path) -> HashMap<String, String> {
    let session_index_path = runtime_home.join("session_index.jsonl");
    let Ok(file) = File::open(session_index_path) else {
        return HashMap::new();
    };

    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<CodexSessionIndexEntry>(&line).ok())
        .filter_map(|entry| entry.thread_name.map(|title| (entry.id, title)))
        .collect()
}

fn extract_codex_session_id(file_name: &str) -> Option<String> {
    file_name
        .strip_prefix("rollout-")?
        .strip_suffix(".jsonl")?
        .rsplit_once('-')
        .map(|(_, session_id)| session_id.to_string())
}

fn read_codex_session_meta_cwd(path: &Path) -> Option<String> {
    let json = read_first_json_line(path)?;
    json_str(&json, &["payload", "cwd"]).map(str::to_string)
}

fn read_codex_first_user_prompt_title(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(envelope) = serde_json::from_str::<CodexRolloutEnvelope>(&line) else {
            continue;
        };
        if envelope.entry_type != "response_item" {
            continue;
        }

        let payload = &envelope.payload;
        if json_str(payload, &["type"]) != Some("message")
            || json_str(payload, &["role"]) != Some("user")
        {
            continue;
        }

        let Some(content) = payload.get("content").and_then(|value| value.as_array()) else {
            continue;
        };
        if let Some(title) = first_text_content_title(content, "input_text") {
            return Some(title);
        }
    }

    None
}

fn read_pi_session_name(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if json_str(&json, &["type"]) == Some("session_info") {
            if let Some(name) = json_str(&json, &["name"]) {
                return Some(normalize_title_whitespace(name));
            }
        }
    }

    None
}

fn read_pi_first_user_prompt_title(path: &Path) -> Option<String> {
    read_message_first_user_prompt_title(path, MessageSchema::Pi)
}

#[derive(Default)]
struct ClaudeSessionMetadata {
    session_id: Option<String>,
    cwd: Option<String>,
}

fn read_claude_session_metadata(path: &Path) -> Option<ClaudeSessionMetadata> {
    let file = File::open(path).ok()?;
    let mut metadata = ClaudeSessionMetadata::default();

    for line in BufReader::new(file).lines().map_while(Result::ok).take(200) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if metadata.session_id.is_none() {
            metadata.session_id = json_str(&json, &["sessionId"]).map(str::to_string);
        }
        if metadata.cwd.is_none() {
            metadata.cwd = json_str(&json, &["cwd"]).map(str::to_string);
        }
        if metadata.session_id.is_some() && metadata.cwd.is_some() {
            return Some(metadata);
        }
    }

    if metadata.session_id.is_some() || metadata.cwd.is_some() {
        Some(metadata)
    } else {
        None
    }
}

fn read_claude_ai_title(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut title = None;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if json_str(&json, &["type"]) == Some("ai-title") {
            title = json_str(&json, &["aiTitle"]).map(normalize_title_whitespace);
        }
    }

    title
}

fn read_claude_first_user_prompt_title(path: &Path) -> Option<String> {
    read_message_first_user_prompt_title(path, MessageSchema::Claude)
}

#[derive(Clone, Copy)]
enum MessageSchema {
    Claude,
    Pi,
}

fn read_message_first_user_prompt_title(path: &Path, schema: MessageSchema) -> Option<String> {
    let file = File::open(path).ok()?;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if json_str(&json, &["type"]) != Some("message") {
            continue;
        }

        let message = json.get("message")?;
        if json_str(message, &["role"]) != Some("user") {
            continue;
        }
        if matches!(schema, MessageSchema::Claude)
            && json.get("isMeta").and_then(|value| value.as_bool()) == Some(true)
        {
            continue;
        }

        let Some(title) = message_content_title(message.get("content")?) else {
            continue;
        };
        if !is_metadata_prompt(&title) {
            return Some(title);
        }
    }

    None
}

fn message_content_title(content: &serde_json::Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        let title = derive_session_title(text);
        return (!title.is_empty()).then_some(title);
    }

    let items = content.as_array()?;
    first_text_content_title(items, "text")
}

fn first_text_content_title(items: &[serde_json::Value], text_type: &str) -> Option<String> {
    for item in items {
        if json_str(item, &["type"]) != Some(text_type) {
            continue;
        }
        let Some(text) = json_str(item, &["text"]) else {
            continue;
        };
        let title = derive_session_title(text);
        if !title.is_empty() && !is_metadata_prompt(&title) {
            return Some(title);
        }
    }

    None
}

fn collect_codex_read_files(
    payload: &serde_json::Value,
    workspace_root: Option<&Path>,
    timestamp_ms: u64,
    read_files: &mut HashMap<String, u64>,
) {
    if json_str(payload, &["type"]) != Some("function_call") {
        return;
    }
    if json_str(payload, &["name"]) != Some("exec_command") {
        return;
    }

    let Some(arguments) = json_str(payload, &["arguments"]) else {
        return;
    };
    let Ok(exec_arguments) = serde_json::from_str::<CodexExecCommandArguments>(arguments) else {
        return;
    };
    let command_root = exec_arguments
        .workdir
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| workspace_root.map(Path::to_path_buf));

    collect_shell_read_files(
        &exec_arguments.cmd,
        command_root.as_deref(),
        timestamp_ms,
        read_files,
    );
}

fn collect_codex_written_files(
    payload: &serde_json::Value,
    timestamp_ms: u64,
    edited_files: &mut HashMap<String, u64>,
    deleted_files: &mut HashMap<String, u64>,
) {
    if json_str(payload, &["type"]) != Some("patch_apply_end") {
        return;
    }

    let Some(changes) = payload.get("changes").and_then(|value| value.as_object()) else {
        return;
    };

    for (path, change) in changes {
        let Some(change_type) = json_str(change, &["type"]) else {
            continue;
        };

        match change_type {
            "delete" => insert_activity_path(deleted_files, path, None, timestamp_ms),
            "add" | "update" | "move" => {
                insert_activity_path(edited_files, path, None, timestamp_ms)
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
enum ToolSchema {
    Claude,
    Pi,
}

fn read_tool_call_file_activity(
    transcript_path: &Path,
    cwd: Option<&str>,
    schema: ToolSchema,
) -> Result<ActivityAccumulator, String> {
    let file = File::open(transcript_path).map_err(|error| {
        format!(
            "failed to open agent transcript {}: {error}",
            transcript_path.display()
        )
    })?;
    let workspace_root = cwd.map(PathBuf::from);
    let mut activity = ActivityAccumulator::default();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let timestamp_ms = entry_timestamp_ms(&json);

        if !matches!(json_str(&json, &["type"]), Some("message" | "assistant")) {
            continue;
        }
        let Some(message) = json.get("message") else {
            continue;
        };
        if json_str(message, &["role"]) != Some("assistant") {
            continue;
        }
        let Some(content) = message.get("content").and_then(|value| value.as_array()) else {
            continue;
        };

        for item in content {
            collect_tool_call_activity(
                item,
                schema,
                workspace_root.as_deref(),
                timestamp_ms,
                &mut activity,
            );
        }
    }

    Ok(activity)
}

fn collect_tool_call_activity(
    item: &serde_json::Value,
    schema: ToolSchema,
    workspace_root: Option<&Path>,
    timestamp_ms: u64,
    activity: &mut ActivityAccumulator,
) {
    let Some(tool_name) = tool_call_name(item, schema) else {
        return;
    };
    let Some(arguments) = tool_call_arguments(item, schema) else {
        return;
    };

    match normalize_tool_name(tool_name).as_str() {
        "read" => {
            if let Some(file_path) = tool_file_path(arguments) {
                insert_activity_path(
                    &mut activity.read_files,
                    file_path,
                    workspace_root,
                    timestamp_ms,
                );
            }
        }
        "write" | "edit" | "multiedit" | "notebookedit" => {
            if let Some(file_path) = tool_file_path(arguments) {
                insert_activity_path(
                    &mut activity.edited_files,
                    file_path,
                    workspace_root,
                    timestamp_ms,
                );
            }
        }
        "bash" => {
            if let Some(command) = json_str(arguments, &["command"]) {
                let command_root = json_str(arguments, &["workdir"])
                    .or_else(|| json_str(arguments, &["cwd"]))
                    .map(PathBuf::from)
                    .or_else(|| workspace_root.map(Path::to_path_buf));
                collect_shell_read_files(
                    command,
                    command_root.as_deref(),
                    timestamp_ms,
                    &mut activity.read_files,
                );
                collect_shell_deleted_files(
                    command,
                    command_root.as_deref(),
                    timestamp_ms,
                    &mut activity.deleted_files,
                );
            }
        }
        _ => {}
    }
}

fn tool_call_name(item: &serde_json::Value, schema: ToolSchema) -> Option<&str> {
    match schema {
        ToolSchema::Claude => {
            if json_str(item, &["type"]) == Some("tool_use") {
                json_str(item, &["name"])
            } else {
                None
            }
        }
        ToolSchema::Pi => {
            if json_str(item, &["type"]) == Some("toolCall") {
                json_str(item, &["name"])
            } else {
                None
            }
        }
    }
}

fn tool_call_arguments(item: &serde_json::Value, schema: ToolSchema) -> Option<&serde_json::Value> {
    match schema {
        ToolSchema::Claude => item.get("input"),
        ToolSchema::Pi => item.get("arguments"),
    }
}

fn normalize_tool_name(tool_name: &str) -> String {
    tool_name
        .chars()
        .filter(|character| *character != '_' && *character != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

fn tool_file_path(arguments: &serde_json::Value) -> Option<&str> {
    json_str(arguments, &["file_path"])
        .or_else(|| json_str(arguments, &["filePath"]))
        .or_else(|| json_str(arguments, &["path"]))
}

fn collect_shell_read_files(
    command: &str,
    command_root: Option<&Path>,
    timestamp_ms: u64,
    read_files: &mut HashMap<String, u64>,
) {
    let Some(command_root) = command_root else {
        return;
    };

    for token in shell_like_tokens(command) {
        if let Some(path) = normalize_existing_activity_path(&token, command_root) {
            insert_activity_path(read_files, &path, None, timestamp_ms);
        }
    }
}

fn collect_shell_deleted_files(
    command: &str,
    command_root: Option<&Path>,
    timestamp_ms: u64,
    deleted_files: &mut HashMap<String, u64>,
) {
    let Some(command_root) = command_root else {
        return;
    };
    let tokens = shell_like_tokens(command);

    for window in tokens.windows(2) {
        if !matches!(
            window.first().map(String::as_str),
            Some("rm") | Some("unlink")
        ) {
            continue;
        }
        let Some(path) = normalize_written_activity_path(&window[1], command_root.to_str()) else {
            continue;
        };
        insert_activity_path(deleted_files, &path, None, timestamp_ms);
    }
}

fn insert_activity_path(
    activity: &mut HashMap<String, u64>,
    path: &str,
    workspace_root: Option<&Path>,
    timestamp_ms: u64,
) {
    let normalized_path = workspace_root
        .and_then(|root| normalize_written_activity_path(path, root.to_str()))
        .unwrap_or_else(|| path.to_string());

    activity
        .entry(normalized_path)
        .and_modify(|current| *current = (*current).max(timestamp_ms))
        .or_insert(timestamp_ms);
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

fn normalize_existing_activity_path(token: &str, workspace_root: &Path) -> Option<String> {
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

fn resolve_impacted_file_relations(
    cwd: Option<&str>,
    edited_files: &HashMap<String, u64>,
    deleted_files: &HashMap<String, u64>,
) -> Result<Vec<AgentSessionImpactedFileRelation>, String> {
    let Some(workspace_root) = cwd.map(PathBuf::from) else {
        return Ok(Vec::new());
    };

    let changed_files = edited_files
        .keys()
        .chain(deleted_files.keys())
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    find_impacted_file_relations(&workspace_root, &changed_files).map(|relations| {
        relations
            .into_iter()
            .map(agent_impacted_file_relation_from_workspace)
            .collect()
    })
}

fn agent_impacted_file_relation_from_workspace(
    relation: WorkspaceImpactedFileRelation,
) -> AgentSessionImpactedFileRelation {
    AgentSessionImpactedFileRelation {
        changed_file: relation.changed_file,
        impacted_file: relation.impacted_file,
        import_specifier: relation.import_specifier,
    }
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

fn read_agent_session_file_diff(
    file_path: &Path,
    cwd: Option<&str>,
) -> Result<AgentSessionFileDiff, String> {
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

    Ok(AgentSessionFileDiff {
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

fn collect_git_index_paths_from_sessions(sessions: &[AgentSessionSummary]) -> Vec<PathBuf> {
    let mut repo_roots = BTreeSet::<PathBuf>::new();

    for session in sessions {
        let Some(cwd) = &session.cwd else {
            continue;
        };
        let Some(repo_root) = resolve_git_repo_root_from_path(Path::new(cwd)) else {
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

fn watch_id_for_provider_path(provider: AgentSessionProvider, runtime_home_path: &Path) -> String {
    format!(
        "{}-session-watch:{}",
        provider_key(provider),
        runtime_home_path.display()
    )
}

fn is_relevant_watch_path(
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

fn normalize_watch_event_path(path: &Path) -> PathBuf {
    PathBuf::from(normalize_absolute_activity_path(path))
}

fn stop_existing_watch(state: &AgentSessionWatchState, watch_id: &str) {
    if let Ok(mut watches) = state.watches.lock() {
        watches.remove(watch_id);
    }
}

fn emit_session_watch_event(app: &AppHandle, payload: SessionWatchEventPayload) {
    let _ = app.emit(AGENT_SESSION_WATCH_EVENT, payload);
}

fn resolve_existing_dir(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("runtime home does not exist: {}", path.display()));
    }

    if !path.is_dir() {
        return Err(format!(
            "runtime home is not a directory: {}",
            path.display()
        ));
    }

    path.canonicalize()
        .map_err(|error| format!("failed to canonicalize runtime home: {error}"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
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

fn payload_timestamp_ms(payload: &serde_json::Value) -> u64 {
    json_str(payload, &["timestamp"])
        .and_then(timestamp_string_to_ms)
        .unwrap_or_default()
}

fn entry_timestamp_ms(entry: &serde_json::Value) -> u64 {
    json_str(entry, &["timestamp"])
        .and_then(timestamp_string_to_ms)
        .unwrap_or_default()
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
        normalize_absolute_activity_path, AgentSessionProtocol, AgentSessionProvider,
        ClaudeSessionProtocol, PiSessionProtocol,
    };
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn pi_protocol_lists_sessions_and_extracts_file_activity() {
        let temp_dir = create_temp_dir("pi-protocol");
        let workspace = temp_dir.join("workspace");
        fs::create_dir_all(&workspace).expect("create workspace");
        let read_path = workspace.join("README.md");
        let edited_path = workspace.join("src.ts");
        fs::write(&read_path, "hello").expect("write read file");
        fs::write(&edited_path, "old").expect("write edited file");
        init_git_repo(&workspace);
        fs::write(&edited_path, "new").expect("modify edited file");

        let sessions_dir = temp_dir.join("sessions").join("--tmp-workspace--");
        fs::create_dir_all(&sessions_dir).expect("create sessions dir");
        let transcript_path = sessions_dir.join("2026-07-05T00-00-00Z_session.jsonl");
        let mut transcript = File::create(&transcript_path).expect("create transcript");
        writeln!(
            transcript,
            r#"{{"type":"session","version":3,"id":"pi-session","timestamp":"2026-07-05T00:00:00Z","cwd":"{}"}}"#,
            workspace.display()
        )
        .expect("write header");
        writeln!(
            transcript,
            r#"{{"type":"message","id":"1","parentId":null,"timestamp":"2026-07-05T00:00:01Z","message":{{"role":"user","content":[{{"type":"text","text":"Implement the thing"}}]}}}}"#
        )
        .expect("write user");
        writeln!(
            transcript,
            r#"{{"type":"message","id":"2","parentId":"1","timestamp":"2026-07-05T00:00:02Z","message":{{"role":"assistant","content":[{{"type":"toolCall","name":"read","arguments":{{"path":"README.md"}}}},{{"type":"toolCall","name":"write","arguments":{{"path":"src.ts"}}}}]}}}}"#
        )
        .expect("write assistant");

        let protocol = PiSessionProtocol;
        let sessions = protocol.list_sessions(&temp_dir);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider, AgentSessionProvider::Pi);
        assert_eq!(sessions[0].title, "Implement the thing");

        let activity = protocol
            .read_file_activity(
                &transcript_path,
                Some(workspace.to_str().expect("utf8 workspace")),
            )
            .expect("read activity");
        assert_eq!(
            activity.read_files,
            vec![normalize_absolute_activity_path(&read_path)]
        );
        assert_eq!(
            activity.edited_files,
            vec![normalize_absolute_activity_path(&edited_path)]
        );
    }

    #[test]
    fn claude_protocol_lists_sessions_and_extracts_file_activity() {
        let temp_dir = create_temp_dir("claude-protocol");
        let workspace = temp_dir.join("workspace");
        fs::create_dir_all(&workspace).expect("create workspace");
        let read_path = workspace.join("README.md");
        let edited_path = workspace.join("src.ts");
        fs::write(&read_path, "hello").expect("write read file");
        fs::write(&edited_path, "old").expect("write edited file");
        init_git_repo(&workspace);
        fs::write(&edited_path, "new").expect("modify edited file");

        let projects_dir = temp_dir.join("projects").join("-tmp-workspace");
        fs::create_dir_all(&projects_dir).expect("create projects dir");
        let transcript_path = projects_dir.join("claude-session.jsonl");
        let mut transcript = File::create(&transcript_path).expect("create transcript");
        writeln!(
            transcript,
            r#"{{"parentUuid":null,"type":"user","message":{{"role":"user","content":"Fix the bug"}},"uuid":"1","timestamp":"2026-07-05T00:00:01Z","cwd":"{}","sessionId":"claude-session"}}"#,
            workspace.display()
        )
        .expect("write user");
        writeln!(
            transcript,
            r#"{{"type":"ai-title","aiTitle":"Fix bug title","sessionId":"claude-session"}}"#
        )
        .expect("write title");
        let assistant_entry = serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "name": "Read",
                        "input": { "file_path": read_path.display().to_string() }
                    },
                    {
                        "type": "tool_use",
                        "name": "Edit",
                        "input": { "file_path": edited_path.display().to_string() }
                    }
                ]
            },
            "uuid": "2",
            "timestamp": "2026-07-05T00:00:02Z",
            "cwd": workspace.display().to_string(),
            "sessionId": "claude-session"
        });
        writeln!(transcript, "{assistant_entry}").expect("write assistant");

        let protocol = ClaudeSessionProtocol;
        let sessions = protocol.list_sessions(&temp_dir);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider, AgentSessionProvider::Claude);
        assert_eq!(sessions[0].title, "Fix bug title");

        let activity = protocol
            .read_file_activity(
                &transcript_path,
                Some(workspace.to_str().expect("utf8 workspace")),
            )
            .expect("read activity");
        assert_eq!(
            activity.read_files,
            vec![normalize_absolute_activity_path(&read_path)]
        );
        assert_eq!(
            activity.edited_files,
            vec![normalize_absolute_activity_path(&edited_path)]
        );
    }

    #[test]
    fn provider_watch_filters_match_expected_paths() {
        let codex_home = Path::new("/tmp/codex-home");
        let claude_home = Path::new("/tmp/claude-home");
        let pi_home = Path::new("/tmp/pi-home");

        assert!(AgentSessionProvider::Codex
            .protocol()
            .is_relevant_session_path(
                &codex_home.join("sessions/a/rollout-2026-test.jsonl"),
                codex_home
            ));
        assert!(AgentSessionProvider::Claude
            .protocol()
            .is_relevant_session_path(
                &claude_home.join("projects/-tmp/session.jsonl"),
                claude_home
            ));
        assert!(AgentSessionProvider::Pi
            .protocol()
            .is_relevant_session_path(&pi_home.join("sessions/--tmp--/session.jsonl"), pi_home));
        assert!(!AgentSessionProvider::Pi
            .protocol()
            .is_relevant_session_path(&pi_home.join("auth.json"), pi_home));
    }

    fn create_temp_dir(label: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("coding-agent-va-{label}-{unique}"));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        temp_dir
    }

    fn init_git_repo(repo_dir: &Path) {
        run_git(repo_dir, &["init"]);
        run_git(repo_dir, &["config", "user.email", "agent@example.com"]);
        run_git(repo_dir, &["config", "user.name", "Agent"]);
        run_git(repo_dir, &["add", "."]);
        run_git(repo_dir, &["commit", "-m", "initial"]);
    }

    fn run_git(repo_dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .arg("-C")
            .arg(repo_dir)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
