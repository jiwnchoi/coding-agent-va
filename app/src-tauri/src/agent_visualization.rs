use crate::indexer::{ArchitectureGraph, WorkspaceIndexer};
use crate::session_watch::{plan_codex_session_watch, SessionWatchPlan};
use chrono::{DateTime, Utc};
use regex::Regex;
use rusqlite::{Connection, OpenFlags, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use walkdir::WalkDir;

const DEFAULT_EVENT_LIMIT: usize = 160;
const SESSION_WATCH_EVENT_NAME: &str = "codex-session-watch-event";

#[derive(Default)]
pub struct VisualBridgeState {
    visual_events: Mutex<Vec<VisualAgentEvent>>,
}

pub fn manage_visual_bridge_state(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.manage(VisualBridgeState::default())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerBootstrap {
    pub workspace_path: String,
    pub runtime_home_candidates: Vec<RuntimeHomeCandidate>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHomeCandidate {
    pub path: String,
    pub source: String,
    pub exists: bool,
    pub score: u32,
    pub artifact_count: u32,
    pub workspace_thread_count: u32,
    pub reason: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionVisualizationSnapshot {
    pub runtime_home: String,
    pub workspace_path: String,
    pub source_mode: String,
    pub event_channel: String,
    pub generated_at_ms: u64,
    pub watch_plan: Option<SessionWatchPlan>,
    pub sessions: Vec<CodexSessionSummary>,
    pub active_session_id: Option<String>,
    pub events: Vec<NormalizedSessionEvent>,
    pub focus_signals: Vec<FocusSignal>,
    pub visual_agent_events: Vec<VisualAgentEvent>,
    pub change_clusters: Vec<ChangeCluster>,
    pub graph: ArchitectureGraph,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionSummary {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub rollout_path: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub source: String,
    pub model_provider: String,
    pub git_branch: Option<String>,
    pub preview: String,
    pub status: String,
    pub relevance_score: i64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventKind {
    UserMessage,
    AssistantMessage,
    ToolCall,
    ToolOutput,
    Command,
    Patch,
    Watch,
    System,
    Error,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSessionEvent {
    pub id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub kind: SessionEventKind,
    pub timestamp_ms: u64,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub path_mentions: Vec<String>,
    pub command: Option<String>,
    pub raw_type: Option<String>,
    pub evidence: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, Hash)]
pub enum FocusKind {
    #[serde(rename = "view_focus")]
    View,
    #[serde(rename = "edit_focus")]
    Edit,
    #[serde(rename = "context_focus")]
    Context,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSignal {
    pub id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub kind: FocusKind,
    pub path: Option<String>,
    pub symbol: Option<String>,
    pub source: String,
    pub score: f64,
    pub timestamp_ms: u64,
    pub evidence: String,
    pub evidence_event_id: String,
    pub node_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualPhase {
    BeforeEdit,
    AfterEdit,
    Checkpoint,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualAgentEventKind {
    ChangeBoundary,
    Relationship,
    RiskMarker,
    DecisionMarker,
    ExternalContextMarker,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualStyle {
    Highlight,
    Group,
    Badge,
    Edge,
    TimelineMarker,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualAgentEventInput {
    pub id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub phase: VisualPhase,
    pub kind: VisualAgentEventKind,
    pub label: String,
    pub visual_target_hints: Option<Vec<String>>,
    pub visual_style: Option<VisualStyle>,
    pub summary: Option<String>,
    pub related_hints: Option<Vec<String>>,
    pub metadata: Option<BTreeMap<String, Value>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualAgentEvent {
    pub id: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub phase: VisualPhase,
    pub kind: VisualAgentEventKind,
    pub label: String,
    pub visual_target_hints: Vec<String>,
    pub visual_style: Option<VisualStyle>,
    pub summary: Option<String>,
    pub related_hints: Vec<String>,
    pub metadata: BTreeMap<String, Value>,
    pub timestamp_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeIntent {
    Bugfix,
    Feature,
    Refactor,
    Cleanup,
    Investigation,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    Forming,
    Active,
    Complete,
    Stale,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeCluster {
    pub id: String,
    pub thread_id: String,
    pub turn_ids: Vec<String>,
    pub title: String,
    pub intent: Option<ChangeIntent>,
    pub status: ChangeStatus,
    pub node_ids: Vec<String>,
    pub focus_signal_ids: Vec<String>,
    pub visual_agent_event_ids: Vec<String>,
    pub evidence_event_ids: Vec<String>,
    pub summary: Option<String>,
}

#[tauri::command]
pub fn get_visualizer_bootstrap() -> Result<VisualizerBootstrap, String> {
    let workspace_path = default_workspace_path()?;
    let runtime_home_candidates = discover_codex_runtime_homes(Some(workspace_path.clone()))?;

    Ok(VisualizerBootstrap {
        workspace_path,
        runtime_home_candidates,
    })
}

#[tauri::command]
pub fn discover_codex_runtime_homes(
    workspace_path: Option<String>,
) -> Result<Vec<RuntimeHomeCandidate>, String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for (source, value) in [
        ("CODEX_HOME", env::var("CODEX_HOME").ok()),
        ("CODEX_RUNTIME_HOME", env::var("CODEX_RUNTIME_HOME").ok()),
    ] {
        if let Some(path) = value {
            push_runtime_home_candidate(&mut candidates, &mut seen, path, source, &workspace_path);
        }
    }

    if let Ok(home) = env::var("HOME") {
        push_runtime_home_candidate(
            &mut candidates,
            &mut seen,
            Path::new(&home).join(".codex").display().to_string(),
            "default_home",
            &workspace_path,
        );
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });

    Ok(candidates)
}

#[tauri::command]
pub fn load_current_codex_visualization(
    state: State<'_, VisualBridgeState>,
    runtime_home: Option<String>,
) -> Result<SessionVisualizationSnapshot, String> {
    let workspace_path = default_workspace_path()?;
    let runtime_home = match runtime_home {
        Some(value) => value,
        None => discover_codex_runtime_homes(Some(workspace_path.clone()))?
            .into_iter()
            .next()
            .map(|candidate| candidate.path)
            .ok_or_else(|| "no Codex runtime home candidates found".to_string())?,
    };

    load_codex_visualization(
        state,
        runtime_home,
        workspace_path,
        Some(DEFAULT_EVENT_LIMIT),
    )
}

#[tauri::command]
pub fn load_codex_visualization(
    state: State<'_, VisualBridgeState>,
    runtime_home: String,
    workspace_path: String,
    event_limit: Option<usize>,
) -> Result<SessionVisualizationSnapshot, String> {
    let runtime_home_path = canonical_dir(&runtime_home, "runtime home")?;
    let workspace_path = canonical_dir(&workspace_path, "workspace")?;
    let event_limit = event_limit.unwrap_or(DEFAULT_EVENT_LIMIT);
    let mut diagnostics = Vec::new();

    let watch_plan = match plan_codex_session_watch(runtime_home_path.display().to_string()) {
        Ok(plan) => Some(plan),
        Err(error) => {
            diagnostics.push(error);
            None
        }
    };

    let mut sessions =
        read_sqlite_sessions(&runtime_home_path, &workspace_path, 24, &mut diagnostics)
            .unwrap_or_default();
    if sessions.is_empty() {
        diagnostics
            .push("state_5.sqlite did not yield sessions; scanning rollout files".to_string());
        sessions = scan_rollout_sessions(&runtime_home_path, &workspace_path, 24);
    }

    let active_session_id = sessions.first().map(|session| session.id.clone());
    let active_session = active_session_id
        .as_ref()
        .and_then(|id| sessions.iter().find(|session| &session.id == id));
    let mut events = active_session
        .and_then(|session| session.rollout_path.as_deref())
        .map(|path| read_rollout_events(path, event_limit, active_session_id.as_deref()))
        .transpose()?
        .unwrap_or_default();

    if events.is_empty() {
        events = current_workspace_fallback_events(
            &workspace_path,
            active_session_id
                .as_deref()
                .unwrap_or("current-workspace-session"),
        );
        diagnostics.push("No rollout events were available for the selected session; showing current workspace fallback events".to_string());
    }

    let graph = WorkspaceIndexer::index(&workspace_path)?;
    let focus_signals = build_focus_signals(&events, &workspace_path, &graph);
    let stored_visual_events = state
        .visual_events
        .lock()
        .map_err(|_| "failed to lock visual bridge state".to_string())?
        .clone();
    let visual_agent_events = build_visual_agent_events(
        active_session_id.as_deref(),
        &events,
        &focus_signals,
        stored_visual_events,
    );
    let change_clusters = build_change_clusters(
        active_session_id.as_deref(),
        &focus_signals,
        &visual_agent_events,
    );

    Ok(SessionVisualizationSnapshot {
        runtime_home: runtime_home_path.display().to_string(),
        workspace_path: workspace_path.display().to_string(),
        source_mode: "runtime_home".to_string(),
        event_channel: SESSION_WATCH_EVENT_NAME.to_string(),
        generated_at_ms: now_timestamp_ms(),
        watch_plan,
        sessions,
        active_session_id,
        events,
        focus_signals,
        visual_agent_events,
        change_clusters,
        graph,
        diagnostics,
    })
}

#[tauri::command]
pub fn record_visual_agent_event(
    state: State<'_, VisualBridgeState>,
    event: VisualAgentEventInput,
) -> Result<VisualAgentEvent, String> {
    let event = normalize_visual_agent_event(event, None);
    state
        .visual_events
        .lock()
        .map_err(|_| "failed to lock visual bridge state".to_string())?
        .push(event.clone());
    Ok(event)
}

fn default_workspace_path() -> Result<String, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|path| path.display().to_string())
        .ok_or_else(|| "failed to derive workspace path from Cargo manifest".to_string())
}

fn canonical_dir(path: &str, label: &str) -> Result<PathBuf, String> {
    let path = expand_home(path);
    if !path.exists() {
        return Err(format!("{label} does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("{label} is not a directory: {}", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("failed to canonicalize {label}: {error}"))
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    PathBuf::from(path)
}

fn push_runtime_home_candidate(
    candidates: &mut Vec<RuntimeHomeCandidate>,
    seen: &mut HashSet<String>,
    path: String,
    source: &str,
    workspace_path: &Option<String>,
) {
    let path = expand_home(&path);
    let path_string = path.display().to_string();
    if !seen.insert(path_string.clone()) {
        return;
    }

    let exists = path.is_dir();
    let artifact_count = [
        path.join("state_5.sqlite"),
        path.join("state_5.sqlite-wal"),
        path.join("history.jsonl"),
        path.join("sessions"),
    ]
    .iter()
    .filter(|artifact| artifact.exists())
    .count() as u32;
    let workspace_thread_count = workspace_path
        .as_ref()
        .and_then(|workspace| count_workspace_threads(&path, workspace).ok())
        .unwrap_or_default();
    let score = if exists { 10 } else { 0 } + artifact_count * 10 + workspace_thread_count * 20;
    let reason = if workspace_thread_count > 0 {
        format!("{workspace_thread_count} thread(s) match the current workspace")
    } else if artifact_count > 0 {
        format!("{artifact_count} expected runtime artifact(s) found")
    } else {
        "candidate path has no recognized Codex runtime artifacts yet".to_string()
    };

    candidates.push(RuntimeHomeCandidate {
        path: path_string,
        source: source.to_string(),
        exists,
        score,
        artifact_count,
        workspace_thread_count,
        reason,
    });
}

fn count_workspace_threads(runtime_home: &Path, workspace_path: &str) -> Result<u32, String> {
    let sqlite_path = runtime_home.join("state_5.sqlite");
    if !sqlite_path.exists() {
        return Ok(0);
    }

    let connection = open_sqlite_readonly(&sqlite_path)?;
    let count: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM threads WHERE archived = 0 AND cwd = ?1",
            [workspace_path],
            |row| row.get(0),
        )
        .unwrap_or_default();
    Ok(count)
}

fn read_sqlite_sessions(
    runtime_home: &Path,
    workspace_path: &Path,
    limit: usize,
    diagnostics: &mut Vec<String>,
) -> Result<Vec<CodexSessionSummary>, String> {
    let sqlite_path = runtime_home.join("state_5.sqlite");
    if !sqlite_path.exists() {
        return Ok(Vec::new());
    }

    let connection = open_sqlite_readonly(&sqlite_path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, title, cwd, rollout_path, \
             COALESCE(created_at_ms, created_at * 1000, 0), \
             COALESCE(updated_at_ms, updated_at * 1000, 0), \
             source, model_provider, git_branch, preview, \
             COALESCE(recency_at_ms, updated_at_ms, updated_at * 1000, 0) \
             FROM threads \
             WHERE archived = 0 \
             ORDER BY CASE WHEN cwd = ?1 THEN 0 ELSE 1 END, recency_at_ms DESC, updated_at_ms DESC \
             LIMIT ?2",
        )
        .map_err(|error| format!("failed to prepare session query: {error}"))?;

    let rows = statement
        .query_map(
            (workspace_path.display().to_string(), limit as i64),
            |row| session_from_row(row, workspace_path),
        )
        .map_err(|error| format!("failed to query Codex sessions: {error}"))?;

    let mut sessions = Vec::new();
    for row in rows {
        match row {
            Ok(session) => sessions.push(session),
            Err(error) => diagnostics.push(format!("skipped malformed session row: {error}")),
        }
    }

    Ok(sessions)
}

fn session_from_row(row: &Row<'_>, workspace_path: &Path) -> rusqlite::Result<CodexSessionSummary> {
    let id: String = row.get(0)?;
    let title = compact_text(&row.get::<_, String>(1).unwrap_or_default(), 96);
    let cwd: String = row.get(2)?;
    let rollout_path: Option<String> = row.get(3)?;
    let created_at_ms: u64 = row.get::<_, i64>(4).unwrap_or_default().max(0) as u64;
    let updated_at_ms: u64 = row.get::<_, i64>(5).unwrap_or_default().max(0) as u64;
    let source = row
        .get::<_, String>(6)
        .unwrap_or_else(|_| "unknown".to_string());
    let model_provider = row
        .get::<_, String>(7)
        .unwrap_or_else(|_| "unknown".to_string());
    let git_branch = row.get::<_, Option<String>>(8)?;
    let preview = compact_text(&row.get::<_, String>(9).unwrap_or_default(), 180);
    let recency_at_ms = row.get::<_, i64>(10).unwrap_or_default().max(0) as u64;
    let relevance_score = session_relevance_score(&cwd, workspace_path, recency_at_ms);

    Ok(CodexSessionSummary {
        id,
        title: if title.is_empty() {
            preview.clone()
        } else {
            title
        },
        cwd,
        rollout_path,
        created_at_ms,
        updated_at_ms,
        source,
        model_provider,
        git_branch,
        preview,
        status: session_status(updated_at_ms),
        relevance_score,
    })
}

fn open_sqlite_readonly(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("failed to mark sqlite connection read-only: {error}"))?;
    Ok(connection)
}

fn session_relevance_score(cwd: &str, workspace_path: &Path, recency_at_ms: u64) -> i64 {
    let workspace = workspace_path.display().to_string();
    let workspace_match = if cwd == workspace {
        1_000_000
    } else if workspace.starts_with(cwd) || cwd.starts_with(&workspace) {
        500_000
    } else {
        0
    };
    workspace_match + (recency_at_ms / 1000) as i64
}

fn session_status(updated_at_ms: u64) -> String {
    let age_ms = now_timestamp_ms().saturating_sub(updated_at_ms);
    if age_ms < 5 * 60 * 1000 {
        "active".to_string()
    } else if age_ms < 24 * 60 * 60 * 1000 {
        "recent".to_string()
    } else {
        "stale".to_string()
    }
}

fn scan_rollout_sessions(
    runtime_home: &Path,
    workspace_path: &Path,
    limit: usize,
) -> Vec<CodexSessionSummary> {
    let sessions_dir = runtime_home.join("sessions");
    if !sessions_dir.exists() {
        return Vec::new();
    }

    let mut rollout_paths = WalkDir::new(sessions_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
        })
        .map(|entry| {
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(system_time_ms)
                .unwrap_or_default();
            (modified, entry.path().to_path_buf())
        })
        .collect::<Vec<_>>();

    rollout_paths.sort_by(|left, right| right.0.cmp(&left.0));
    rollout_paths
        .into_iter()
        .take(limit)
        .filter_map(|(updated_at_ms, path)| {
            session_from_rollout_path(path, updated_at_ms, workspace_path)
        })
        .collect()
}

fn session_from_rollout_path(
    path: PathBuf,
    updated_at_ms: u64,
    workspace_path: &Path,
) -> Option<CodexSessionSummary> {
    let file = File::open(&path).ok()?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line).ok()?;
    let value = serde_json::from_str::<Value>(&first_line).ok()?;
    let payload = value.get("payload")?;
    let id = payload
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| rollout_id_from_path(&path))?;
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    Some(CodexSessionSummary {
        id,
        title: "Rollout session".to_string(),
        cwd: cwd.clone(),
        rollout_path: Some(path.display().to_string()),
        created_at_ms: payload
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_timestamp_ms)
            .unwrap_or(updated_at_ms),
        updated_at_ms,
        source: payload
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("rollout")
            .to_string(),
        model_provider: payload
            .get("model_provider")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        git_branch: None,
        preview: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("rollout")
            .to_string(),
        status: session_status(updated_at_ms),
        relevance_score: session_relevance_score(&cwd, workspace_path, updated_at_ms),
    })
}

fn rollout_id_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let id = file_name
        .strip_prefix("rollout-")?
        .strip_suffix(".jsonl")?
        .rsplit('-')
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("-");
    Some(id)
}

fn read_rollout_events(
    rollout_path: &str,
    event_limit: usize,
    fallback_thread_id: Option<&str>,
) -> Result<Vec<NormalizedSessionEvent>, String> {
    let file = File::open(rollout_path)
        .map_err(|error| format!("failed to open rollout {rollout_path}: {error}"))?;
    let reader = BufReader::new(file);
    let mut events = VecDeque::new();
    let mut thread_id = fallback_thread_id.map(str::to_string);

    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| format!("failed to read rollout line: {error}"))?;
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            if let Some(id) = value
                .get("payload")
                .and_then(|payload| payload.get("id"))
                .and_then(Value::as_str)
            {
                thread_id = Some(id.to_string());
            }
        }

        if let Some(event) = normalize_rollout_value(
            &value,
            index,
            rollout_path,
            thread_id.as_deref().unwrap_or("unknown-session"),
        ) {
            events.push_back(event);
            while events.len() > event_limit {
                events.pop_front();
            }
        }
    }

    Ok(events.into_iter().collect())
}

fn normalize_rollout_value(
    value: &Value,
    index: usize,
    rollout_path: &str,
    thread_id: &str,
) -> Option<NormalizedSessionEvent> {
    let raw_type = value.get("type").and_then(Value::as_str)?.to_string();
    let timestamp_ms = value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(parse_timestamp_ms)
        .unwrap_or_else(now_timestamp_ms);
    let null_payload = Value::Null;
    let payload = value.get("payload").unwrap_or(&null_payload);

    let (kind, title, summary, command, turn_id) = match raw_type.as_str() {
        "session_meta" => {
            let cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .unwrap_or("unknown workspace");
            (
                SessionEventKind::System,
                "Session metadata".to_string(),
                format!("Workspace {cwd}"),
                None,
                None,
            )
        }
        "response_item" => response_item_event_fields(payload),
        "event_msg" => event_msg_fields(payload),
        _ => (
            SessionEventKind::System,
            raw_type.clone(),
            compact_text(&payload.to_string(), 240),
            None,
            None,
        ),
    };

    if summary.is_empty() && command.is_none() {
        return None;
    }

    let source_text = [summary.as_str(), command.as_deref().unwrap_or_default()].join(" ");
    let path_mentions = extract_path_mentions(&source_text);

    Some(NormalizedSessionEvent {
        id: format!("{thread_id}:{index}"),
        thread_id: thread_id.to_string(),
        turn_id,
        kind,
        timestamp_ms,
        title,
        summary: compact_text(&summary, 420),
        source: rollout_path.to_string(),
        path_mentions,
        command: command.map(|value| compact_text(&value, 260)),
        raw_type: Some(raw_type),
        evidence: compact_text(&source_text, 260),
    })
}

fn response_item_event_fields(
    payload: &Value,
) -> (
    SessionEventKind,
    String,
    String,
    Option<String>,
    Option<String>,
) {
    let item_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("response_item");
    let turn_id = payload
        .get("call_id")
        .or_else(|| payload.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);

    match item_type {
        "message" => {
            let role = payload
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("assistant");
            let text = extract_message_text(payload);
            let kind = if role == "user" {
                SessionEventKind::UserMessage
            } else {
                SessionEventKind::AssistantMessage
            };
            (kind, format!("{role} message"), text, None, turn_id)
        }
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("tool_call");
            let arguments = payload
                .get("arguments")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    payload
                        .get("arguments")
                        .map(Value::to_string)
                        .unwrap_or_default()
                });
            let command = extract_command_from_arguments(&arguments);
            let kind = if name.contains("apply_patch") {
                SessionEventKind::Patch
            } else if command.is_some() || name.contains("exec_command") {
                SessionEventKind::Command
            } else {
                SessionEventKind::ToolCall
            };
            let summary = match &command {
                Some(command) => command.clone(),
                None => compact_text(&arguments, 240),
            };
            (kind, name.to_string(), summary, command, turn_id)
        }
        "function_call_output" => {
            let output = payload
                .get("output")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (
                SessionEventKind::ToolOutput,
                "Tool output".to_string(),
                output.to_string(),
                None,
                turn_id,
            )
        }
        _ => (
            SessionEventKind::System,
            item_type.to_string(),
            compact_text(&payload.to_string(), 240),
            None,
            turn_id,
        ),
    }
}

fn event_msg_fields(
    payload: &Value,
) -> (
    SessionEventKind,
    String,
    String,
    Option<String>,
    Option<String>,
) {
    let event_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("event");
    let summary = payload
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| compact_text(&payload.to_string(), 220));
    let kind = if event_type.contains("error") || summary.to_lowercase().contains("error") {
        SessionEventKind::Error
    } else {
        SessionEventKind::System
    };
    (kind, event_type.to_string(), summary, None, None)
}

fn extract_message_text(payload: &Value) -> String {
    if let Some(text) = payload.get("text").and_then(Value::as_str) {
        return text.to_string();
    }

    payload
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn extract_command_from_arguments(arguments: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(arguments) {
        if let Some(cmd) = value.get("cmd").and_then(Value::as_str) {
            return Some(cmd.to_string());
        }

        if let Some(tool_uses) = value.get("tool_uses").and_then(Value::as_array) {
            let commands = tool_uses
                .iter()
                .filter_map(|tool_use| {
                    tool_use
                        .get("parameters")
                        .and_then(|parameters| parameters.get("cmd"))
                        .and_then(Value::as_str)
                })
                .take(3)
                .collect::<Vec<_>>();
            if !commands.is_empty() {
                return Some(commands.join(" | "));
            }
        }
    }

    None
}

fn current_workspace_fallback_events(
    workspace_path: &Path,
    thread_id: &str,
) -> Vec<NormalizedSessionEvent> {
    let timestamp_ms = now_timestamp_ms();
    let workspace = workspace_path.display();
    [
        (
            SessionEventKind::UserMessage,
            "User objective",
            "Implement docs/PLANS.md and docs/AGENT_VIS_BRIDGE.md as a working Codex analysis visualizer prototype.",
            None,
            vec!["docs/PLANS.md", "docs/AGENT_VIS_BRIDGE.md"],
        ),
        (
            SessionEventKind::ToolCall,
            "Plan docs read",
            "Read the product plan and bridge taxonomy, then mapped them into runtime discovery, ingestion, focus, visual events, clusters, and graph views.",
            None,
            vec!["docs/PLANS.md", "docs/AGENT_VIS_BRIDGE.md"],
        ),
        (
            SessionEventKind::Patch,
            "Prototype implementation",
            "Added a Tauri bridge module and replaced the placeholder React screen with the observer UI.",
            None,
            vec![
                "app/src-tauri/src/agent_visualization.rs",
                "app/src/App.tsx",
                "app/src/lib/visualizerApi.ts",
            ],
        ),
        (
            SessionEventKind::Watch,
            "Runtime watch plan",
            "Watch targets cover state_5.sqlite, state_5.sqlite-wal, history.jsonl, and sessions rollout files.",
            None,
            vec!["state_5.sqlite", "history.jsonl", "sessions/rollout-current.jsonl"],
        ),
        (
            SessionEventKind::Command,
            "Verification command",
            "Run the repository justfile checks after implementation.",
            Some("just check"),
            vec!["justfile", "tools/justfile.node", "tools/justfile.rust"],
        ),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (kind, title, summary, command, path_mentions))| NormalizedSessionEvent {
            id: format!("{thread_id}:fallback:{index}"),
            thread_id: thread_id.to_string(),
            turn_id: Some(format!("fallback-{index}")),
            kind,
            timestamp_ms: timestamp_ms + index as u64,
            title: title.to_string(),
            summary: summary.to_string(),
            source: workspace.to_string(),
            path_mentions: path_mentions.into_iter().map(str::to_string).collect(),
            command: command.map(str::to_string),
            raw_type: Some("fallback".to_string()),
            evidence: summary.to_string(),
        },
    )
    .collect()
}

fn build_focus_signals(
    events: &[NormalizedSessionEvent],
    workspace_path: &Path,
    graph: &ArchitectureGraph,
) -> Vec<FocusSignal> {
    let graph_paths = graph
        .nodes
        .iter()
        .filter_map(|node| {
            node.path
                .as_ref()
                .map(|path| (path.clone(), node.id.clone()))
        })
        .collect::<HashMap<_, _>>();
    let mut signals = Vec::new();
    let mut seen = HashSet::new();

    for event in events {
        for path in &event.path_mentions {
            let normalized_path = normalize_focus_path(path, workspace_path);
            let kind = match event.kind {
                SessionEventKind::Patch => FocusKind::Edit,
                SessionEventKind::Command | SessionEventKind::ToolCall => FocusKind::View,
                _ => FocusKind::Context,
            };
            let key = format!("{kind:?}:{}:{}", normalized_path, event.id);
            if !seen.insert(key) {
                continue;
            }

            let node_id = graph_paths
                .get(&normalized_path)
                .cloned()
                .or_else(|| graph_paths.get(path).cloned());
            let score = match kind {
                FocusKind::Edit => 0.95,
                FocusKind::View => 0.72,
                FocusKind::Context => 0.42,
            };

            signals.push(FocusSignal {
                id: format!(
                    "focus:{}:{}",
                    signals.len(),
                    stable_fragment(&normalized_path)
                ),
                thread_id: event.thread_id.clone(),
                turn_id: event.turn_id.clone(),
                kind,
                path: Some(normalized_path),
                symbol: None,
                source: focus_source_for_event(event.kind).to_string(),
                score,
                timestamp_ms: event.timestamp_ms,
                evidence: event.evidence.clone(),
                evidence_event_id: event.id.clone(),
                node_id,
            });
        }
    }

    signals.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.timestamp_ms.cmp(&left.timestamp_ms))
    });
    signals.truncate(80);
    signals
}

fn focus_source_for_event(kind: SessionEventKind) -> &'static str {
    match kind {
        SessionEventKind::Patch => "patch",
        SessionEventKind::Command | SessionEventKind::ToolCall => "tool_call",
        SessionEventKind::ToolOutput => "tool_output",
        SessionEventKind::AssistantMessage => "assistant_message",
        SessionEventKind::UserMessage => "user_message",
        SessionEventKind::Watch | SessionEventKind::System | SessionEventKind::Error => {
            "tool_output"
        }
    }
}

fn build_visual_agent_events(
    thread_id: Option<&str>,
    events: &[NormalizedSessionEvent],
    focus_signals: &[FocusSignal],
    mut stored_events: Vec<VisualAgentEvent>,
) -> Vec<VisualAgentEvent> {
    let mut visual_events = Vec::new();
    let thread_id = thread_id.map(str::to_string);
    let top_targets = focus_signals
        .iter()
        .filter_map(|signal| signal.path.clone())
        .take(6)
        .collect::<Vec<_>>();

    if events
        .iter()
        .flat_map(|event| &event.path_mentions)
        .any(|path| path == "docs/PLANS.md" || path == "docs/AGENT_VIS_BRIDGE.md")
    {
        visual_events.push(VisualAgentEvent {
            id: "auto:external-context:planning-docs".to_string(),
            thread_id: thread_id.clone(),
            turn_id: None,
            phase: VisualPhase::Checkpoint,
            kind: VisualAgentEventKind::ExternalContextMarker,
            label: "Planning docs used as bridge input".to_string(),
            visual_target_hints: vec![
                "docs/PLANS.md".to_string(),
                "docs/AGENT_VIS_BRIDGE.md".to_string(),
            ],
            visual_style: Some(VisualStyle::Badge),
            summary: Some(
                "The bridge output is grounded in the product plan and visual event taxonomy."
                    .to_string(),
            ),
            related_hints: Vec::new(),
            metadata: BTreeMap::new(),
            timestamp_ms: now_timestamp_ms(),
        });
    }

    if !top_targets.is_empty() {
        visual_events.push(VisualAgentEvent {
            id: "auto:change-boundary:workspace-focus".to_string(),
            thread_id: thread_id.clone(),
            turn_id: None,
            phase: if focus_signals
                .iter()
                .any(|signal| signal.kind == FocusKind::Edit)
            {
                VisualPhase::AfterEdit
            } else {
                VisualPhase::BeforeEdit
            },
            kind: VisualAgentEventKind::ChangeBoundary,
            label: "Current workspace change unit".to_string(),
            visual_target_hints: top_targets.clone(),
            visual_style: Some(VisualStyle::Group),
            summary: Some(
                "Focus signals group the files currently being read or edited.".to_string(),
            ),
            related_hints: Vec::new(),
            metadata: BTreeMap::new(),
            timestamp_ms: now_timestamp_ms(),
        });
    }

    if top_targets.len() > 1 {
        visual_events.push(VisualAgentEvent {
            id: "auto:relationship:focus-neighborhood".to_string(),
            thread_id,
            turn_id: None,
            phase: VisualPhase::Checkpoint,
            kind: VisualAgentEventKind::Relationship,
            label: "Focus neighborhood".to_string(),
            visual_target_hints: top_targets,
            visual_style: Some(VisualStyle::Edge),
            summary: Some(
                "Recently mentioned paths are linked as the current context graph neighborhood."
                    .to_string(),
            ),
            related_hints: Vec::new(),
            metadata: BTreeMap::new(),
            timestamp_ms: now_timestamp_ms(),
        });
    }

    visual_events.append(&mut stored_events);
    visual_events
}

fn build_change_clusters(
    active_thread_id: Option<&str>,
    focus_signals: &[FocusSignal],
    visual_agent_events: &[VisualAgentEvent],
) -> Vec<ChangeCluster> {
    let thread_id = active_thread_id.unwrap_or("current-workspace-session");
    let mut clusters = Vec::new();
    let mut node_ids = focus_signals
        .iter()
        .filter_map(|signal| signal.node_id.clone())
        .collect::<Vec<_>>();
    node_ids.sort();
    node_ids.dedup();

    let focus_signal_ids = focus_signals
        .iter()
        .take(12)
        .map(|signal| signal.id.clone())
        .collect::<Vec<_>>();
    let evidence_event_ids = focus_signals
        .iter()
        .take(12)
        .map(|signal| signal.evidence_event_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let visual_agent_event_ids = visual_agent_events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();

    if !focus_signal_ids.is_empty() || !visual_agent_event_ids.is_empty() {
        let has_edit = focus_signals
            .iter()
            .any(|signal| signal.kind == FocusKind::Edit);
        let intent = infer_cluster_intent(visual_agent_events);
        let status = infer_cluster_status(focus_signals, visual_agent_events, has_edit);
        clusters.push(ChangeCluster {
            id: format!("cluster:{thread_id}:primary"),
            thread_id: thread_id.to_string(),
            turn_ids: focus_signals
                .iter()
                .filter_map(|signal| signal.turn_id.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect(),
            title: if has_edit {
                "Implementation change cluster".to_string()
            } else {
                "Observation context cluster".to_string()
            },
            intent: Some(intent),
            status,
            node_ids,
            focus_signal_ids,
            visual_agent_event_ids,
            evidence_event_ids,
            summary: Some(
                "Rule-based cluster from same-session path mentions, patch signals, and visual event hints."
                    .to_string(),
            ),
        });
    }

    for event in visual_agent_events
        .iter()
        .filter(|event| event.kind == VisualAgentEventKind::DecisionMarker)
    {
        clusters.push(ChangeCluster {
            id: format!("cluster:{}:{}", thread_id, event.id),
            thread_id: thread_id.to_string(),
            turn_ids: event.turn_id.iter().cloned().collect(),
            title: event.label.clone(),
            intent: Some(ChangeIntent::Investigation),
            status: ChangeStatus::Forming,
            node_ids: Vec::new(),
            focus_signal_ids: Vec::new(),
            visual_agent_event_ids: vec![event.id.clone()],
            evidence_event_ids: Vec::new(),
            summary: event.summary.clone(),
        });
    }

    clusters
}

fn infer_cluster_intent(visual_agent_events: &[VisualAgentEvent]) -> ChangeIntent {
    let text = visual_agent_events
        .iter()
        .map(|event| {
            format!(
                "{} {}",
                event.label,
                event.summary.clone().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    if text.contains("bug") || text.contains("fix") {
        ChangeIntent::Bugfix
    } else if text.contains("refactor") {
        ChangeIntent::Refactor
    } else if text.contains("cleanup") || text.contains("clean") {
        ChangeIntent::Cleanup
    } else if text.contains("investigat") || text.contains("decision") {
        ChangeIntent::Investigation
    } else {
        ChangeIntent::Feature
    }
}

fn infer_cluster_status(
    focus_signals: &[FocusSignal],
    visual_agent_events: &[VisualAgentEvent],
    has_edit: bool,
) -> ChangeStatus {
    if visual_agent_events
        .iter()
        .any(|event| event.phase == VisualPhase::AfterEdit)
    {
        ChangeStatus::Complete
    } else if focus_signals
        .iter()
        .map(|signal| signal.timestamp_ms)
        .max()
        .is_some_and(|latest| now_timestamp_ms().saturating_sub(latest) > 24 * 60 * 60 * 1000)
    {
        ChangeStatus::Stale
    } else if has_edit {
        ChangeStatus::Active
    } else {
        ChangeStatus::Forming
    }
}

fn normalize_visual_agent_event(
    input: VisualAgentEventInput,
    default_thread_id: Option<String>,
) -> VisualAgentEvent {
    VisualAgentEvent {
        id: input
            .id
            .unwrap_or_else(|| format!("visual:{}", now_timestamp_ms())),
        thread_id: input.thread_id.or(default_thread_id),
        turn_id: input.turn_id,
        phase: input.phase,
        kind: input.kind,
        label: input.label,
        visual_target_hints: input.visual_target_hints.unwrap_or_default(),
        visual_style: input.visual_style,
        summary: input.summary,
        related_hints: input.related_hints.unwrap_or_default(),
        metadata: input.metadata.unwrap_or_default(),
        timestamp_ms: now_timestamp_ms(),
    }
}

fn normalize_focus_path(path: &str, workspace_path: &Path) -> String {
    let cleaned = path
        .trim_matches(|character: char| {
            matches!(
                character,
                '`' | '"' | '\'' | ',' | '.' | ':' | ';' | ')' | '(' | '[' | ']'
            )
        })
        .to_string();
    let path = expand_home(&cleaned);
    let absolute = if path.is_absolute() {
        path
    } else {
        workspace_path.join(path)
    };

    absolute
        .canonicalize()
        .unwrap_or(absolute)
        .display()
        .to_string()
}

fn extract_path_mentions(text: &str) -> Vec<String> {
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let regex = PATH_RE.get_or_init(|| {
        Regex::new(r#"(?m)(?:~|\.{1,2}|/[A-Za-z0-9_.-]+|[A-Za-z0-9_.-]+)/(?:[A-Za-z0-9_@+.,:=/-]+)|[A-Za-z0-9_.-]+\.(?:rs|ts|tsx|js|jsx|json|jsonl|md|toml|yml|yaml|css|html|py|sh|sql)"#)
            .expect("valid path mention regex")
    });
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    for matched in regex.find_iter(text) {
        let candidate = matched
            .as_str()
            .trim_matches(|character: char| {
                matches!(
                    character,
                    '`' | '"' | '\'' | ',' | '.' | ':' | ';' | ')' | '(' | '[' | ']'
                )
            })
            .to_string();
        if candidate.contains("://") || candidate.len() < 3 {
            continue;
        }
        if seen.insert(candidate.clone()) {
            paths.push(candidate);
        }
        if paths.len() >= 10 {
            break;
        }
    }

    paths
}

fn compact_text(text: &str, limit: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= limit {
        return normalized;
    }

    let mut compacted = normalized
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    compacted.push_str("...");
    compacted
}

fn stable_fragment(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(32)
        .collect::<String>()
}

fn parse_timestamp_ms(value: &str) -> Option<u64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc).timestamp_millis().max(0) as u64)
}

fn system_time_ms(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        build_focus_signals, current_workspace_fallback_events, extract_path_mentions,
        normalize_rollout_value, SessionEventKind,
    };
    use crate::indexer::ArchitectureGraph;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn extracts_path_mentions_from_mixed_text() {
        let paths = extract_path_mentions(
            "Read docs/PLANS.md, app/src/App.tsx, and /tmp/example/state_5.sqlite.",
        );

        assert!(paths.contains(&"docs/PLANS.md".to_string()));
        assert!(paths.contains(&"app/src/App.tsx".to_string()));
        assert!(paths.contains(&"/tmp/example/state_5.sqlite".to_string()));
    }

    #[test]
    fn normalizes_exec_command_response_item() {
        let value = json!({
            "timestamp": "2026-06-29T12:00:00Z",
            "type": "response_item",
            "payload": {
                "type": "function_call",
                "name": "functions.exec_command",
                "call_id": "call_1",
                "arguments": "{\"cmd\":\"sed -n '1,20p' docs/PLANS.md\"}"
            }
        });

        let event = normalize_rollout_value(&value, 1, "/tmp/rollout.jsonl", "thread")
            .expect("normalize event");

        assert_eq!(event.kind, SessionEventKind::Command);
        assert_eq!(
            event.command.as_deref(),
            Some("sed -n '1,20p' docs/PLANS.md")
        );
        assert!(event.path_mentions.contains(&"docs/PLANS.md".to_string()));
    }

    #[test]
    fn fallback_events_create_focus_signals() {
        let workspace = Path::new("/tmp/workspace");
        let events = current_workspace_fallback_events(workspace, "thread");
        let focus = build_focus_signals(&events, workspace, &ArchitectureGraph::default());

        assert!(focus.iter().any(|signal| signal
            .path
            .as_deref()
            .is_some_and(|path| path.ends_with("docs/PLANS.md"))));
    }
}
