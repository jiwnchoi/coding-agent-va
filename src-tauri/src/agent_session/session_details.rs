use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::indexer::WorkspaceIndexState;
use serde::Deserialize;
use tree_sitter::{Node, Parser};

use super::activity::codex::{collect_codex_read_files, collect_codex_written_files};
use super::activity::{
    collect_tool_call_entry_activity, finish_file_activity, resolve_impacted_file_relations_cached,
    ActivityAccumulator, ToolSchema,
};
use super::time::entry_timestamp_ms;
use super::titles::{is_metadata_prompt, strip_image_attachment_markers};
use super::types::{
    AgentSessionDetails, AgentSessionFileActivity, AgentSessionPromptTurn, AgentSessionProvider,
    AgentSessionTask, AgentSessionTaskStatus,
};

#[cfg(test)]
pub(crate) fn read_session_details(
    provider: AgentSessionProvider,
    provider_session_id: &str,
    transcript_path: &Path,
    runtime_home: &Path,
    cwd: Option<&str>,
) -> Result<AgentSessionDetails, String> {
    read_session_details_cached(
        &WorkspaceIndexState::default(),
        provider,
        provider_session_id,
        transcript_path,
        runtime_home,
        cwd,
    )
}

pub(crate) fn read_session_details_cached(
    index_state: &WorkspaceIndexState,
    provider: AgentSessionProvider,
    provider_session_id: &str,
    transcript_path: &Path,
    runtime_home: &Path,
    cwd: Option<&str>,
) -> Result<AgentSessionDetails, String> {
    let entries = read_json_lines(transcript_path)?;
    let claude_tasks = if matches!(provider, AgentSessionProvider::Claude) {
        read_claude_task_files(provider_session_id, runtime_home)?
    } else {
        HashMap::new()
    };
    let prompt_indexes = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            prompt_text(provider, entry)
                .filter(|prompt| !is_metadata_prompt(prompt))
                .map(|prompt| (index, prompt))
        })
        .collect::<Vec<_>>();
    let mut turns = Vec::with_capacity(prompt_indexes.len());
    let mut interrupted_turns = Vec::with_capacity(prompt_indexes.len());
    let mut session_activity = ActivityAccumulator::default();
    for (position, (start, prompt)) in prompt_indexes.iter().enumerate() {
        let end = prompt_indexes
            .get(position + 1)
            .map(|(index, _)| *index)
            .unwrap_or(entries.len());
        let turn_entries = &entries[*start..end];
        let (turn, activity) = build_turn(
            provider,
            provider_session_id,
            position,
            prompt,
            turn_entries,
            &claude_tasks,
            cwd,
        );
        merge_activity_accumulator(&mut session_activity, activity);
        turns.push(turn);
        interrupted_turns.push(turn_was_interrupted(provider, turn_entries));
    }
    consolidate_session_tasks(&mut turns);
    consolidate_interrupted_turns(&mut turns, &interrupted_turns);
    session_activity.retain_workspace_paths(cwd);
    let context_read_files = session_activity
        .read_files
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut file_activity = finish_file_activity(cwd, session_activity.clone());
    let impacted_relations = resolve_impacted_file_relations_cached(
        index_state,
        cwd,
        &session_activity.edited_files,
        &session_activity.deleted_files,
        &session_activity.edit_fragments,
    )?;
    apply_impacted_relations(
        cwd,
        &mut file_activity,
        &impacted_relations,
        &context_read_files,
    );
    for turn in &mut turns {
        apply_impacted_relations(
            cwd,
            &mut turn.file_activity,
            &impacted_relations,
            &context_read_files,
        );
        for task in &mut turn.tasks {
            apply_impacted_relations(
                cwd,
                &mut task.file_activity,
                &impacted_relations,
                &context_read_files,
            );
        }
    }
    Ok(AgentSessionDetails {
        file_activity,
        turns,
    })
}

fn merge_activity_accumulator(target: &mut ActivityAccumulator, update: ActivityAccumulator) {
    merge_timestamped_paths(&mut target.read_files, update.read_files);
    merge_timestamped_paths(&mut target.edited_files, update.edited_files);
    merge_timestamped_paths(&mut target.deleted_files, update.deleted_files);
    for (path, fragments) in update.edit_fragments {
        let target_fragments = target.edit_fragments.entry(path).or_default();
        for fragment in fragments {
            if !target_fragments.contains(&fragment) {
                target_fragments.push(fragment);
            }
        }
    }
}

fn merge_timestamped_paths(target: &mut HashMap<String, u64>, update: HashMap<String, u64>) {
    for (path, timestamp) in update {
        target
            .entry(path)
            .and_modify(|current| *current = (*current).max(timestamp))
            .or_insert(timestamp);
    }
}

fn apply_impacted_relations(
    cwd: Option<&str>,
    activity: &mut AgentSessionFileActivity,
    session_relations: &[super::types::AgentSessionImpactedFileRelation],
    context_read_files: &[String],
) {
    let changed_files = activity
        .edited_files
        .iter()
        .chain(&activity.deleted_files)
        .filter_map(|path| workspace_relative_path(cwd, path))
        .collect::<HashSet<_>>();
    activity.impacted_relations = session_relations
        .iter()
        .filter(|relation| changed_files.contains(&relation.changed_file))
        .cloned()
        .collect();
    let impacted_files = activity
        .impacted_relations
        .iter()
        .map(|relation| relation.impacted_file.clone())
        .collect::<HashSet<_>>();
    activity.impacted_files = impacted_files.iter().cloned().collect();
    activity.impacted_files.sort();
    let mut contextual_reads = context_read_files
        .iter()
        .filter(|path| {
            workspace_relative_path(cwd, path).is_some_and(|path| impacted_files.contains(&path))
        })
        .filter(|path| !activity.read_files.contains(path))
        .cloned()
        .collect::<Vec<_>>();
    contextual_reads.sort();
    activity.read_files.extend(contextual_reads);
}

fn workspace_relative_path(cwd: Option<&str>, path: &str) -> Option<String> {
    let workspace_root = Path::new(cwd?);
    Path::new(path)
        .strip_prefix(workspace_root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

fn consolidate_interrupted_turns(
    turns: &mut Vec<AgentSessionPromptTurn>,
    interrupted_turns: &[bool],
) {
    let mut consolidated = Vec::<AgentSessionPromptTurn>::with_capacity(turns.len());
    let mut merge_next = false;
    for (turn_index, mut turn) in std::mem::take(turns).into_iter().enumerate() {
        let current_merges_with_next = interrupted_turns
            .get(turn_index)
            .copied()
            .unwrap_or_default()
            || turn.summary.is_none();
        if !merge_next {
            consolidated.push(turn);
            merge_next = current_merges_with_next;
            continue;
        }

        let owner = consolidated.last_mut().expect("merged turn has an owner");
        owner.prompts.append(&mut turn.prompts);
        owner.summary = turn.summary.take();
        owner.tasks.append(&mut turn.tasks);
        merge_file_activity(&mut owner.file_activity, turn.file_activity);
        merge_next = current_merges_with_next;
    }
    *turns = consolidated;
}

fn turn_was_interrupted(provider: AgentSessionProvider, entries: &[serde_json::Value]) -> bool {
    matches!(provider, AgentSessionProvider::Codex)
        && entries.iter().any(|entry| {
            entry.get("type").and_then(serde_json::Value::as_str) == Some("event_msg")
                && entry
                    .get("payload")
                    .and_then(|payload| payload.get("type"))
                    .and_then(serde_json::Value::as_str)
                    == Some("turn_aborted")
        })
}

fn consolidate_session_tasks(turns: &mut [AgentSessionPromptTurn]) {
    let mut owner_by_task_id = HashMap::<String, (usize, usize)>::new();
    let mut generation_by_task_id = HashMap::<String, usize>::new();
    let mut open_task_ids = HashSet::<String>::new();
    for turn_index in 0..turns.len() {
        let inherited_task_ids = open_task_ids.clone();
        let inherited_activity = turns[turn_index].file_activity.clone();
        let mut status_updates = Vec::new();
        let mut task_index = 0;
        while task_index < turns[turn_index].tasks.len() {
            let mut task_id = turns[turn_index].tasks[task_index].id.clone();
            let task_status = turns[turn_index].tasks[task_index].status;
            if let Some(&(owner_turn, owner_task)) = owner_by_task_id.get(&task_id) {
                let owner = &turns[owner_turn].tasks[owner_task];
                if owner.native_id.is_none()
                    && matches!(owner.status, AgentSessionTaskStatus::Completed)
                    && !matches!(task_status, AgentSessionTaskStatus::Completed)
                {
                    let generation = generation_by_task_id.entry(task_id.clone()).or_insert(1);
                    task_id = format!("{task_id}:generation:{generation}");
                    *generation += 1;
                    turns[turn_index].tasks[task_index].id = task_id.clone();
                }
            }
            status_updates.push((task_id.clone(), task_status));
            if let Some(&(owner_turn, owner_task)) = owner_by_task_id.get(&task_id) {
                let update = turns[turn_index].tasks.remove(task_index);
                let owner = &mut turns[owner_turn].tasks[owner_task];
                owner.subject = update.subject;
                owner.description = update.description.or_else(|| owner.description.take());
                owner.active_form = update.active_form.or_else(|| owner.active_form.take());
                owner.status = update.status;
                owner.depends_on = update.depends_on;
                if update.summary.is_some() {
                    owner.summary = update.summary;
                }
                merge_file_activity(&mut owner.file_activity, update.file_activity);
            } else {
                owner_by_task_id.insert(task_id, (turn_index, task_index));
                task_index += 1;
            }
        }

        for task_id in inherited_task_ids {
            if let Some(&(owner_turn, owner_task)) = owner_by_task_id.get(&task_id) {
                merge_file_activity(
                    &mut turns[owner_turn].tasks[owner_task].file_activity,
                    inherited_activity.clone(),
                );
            }
        }

        for (task_id, status) in status_updates {
            match status {
                AgentSessionTaskStatus::Pending | AgentSessionTaskStatus::InProgress => {
                    open_task_ids.insert(task_id);
                }
                AgentSessionTaskStatus::Completed => {
                    open_task_ids.remove(&task_id);
                }
            }
        }
    }
}

fn merge_file_activity(target: &mut AgentSessionFileActivity, update: AgentSessionFileActivity) {
    merge_unique(&mut target.read_files, update.read_files);
    merge_unique(&mut target.edited_files, update.edited_files);
    merge_unique(&mut target.impacted_files, update.impacted_files);
    merge_unique(&mut target.deleted_files, update.deleted_files);
    for relation in update.impacted_relations {
        if !target.impacted_relations.iter().any(|existing| {
            existing.changed_file == relation.changed_file
                && existing.impacted_file == relation.impacted_file
                && existing.import_specifier == relation.import_specifier
        }) {
            target.impacted_relations.push(relation);
        }
    }
}

fn merge_unique(target: &mut Vec<String>, update: Vec<String>) {
    for value in update {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}

fn read_json_lines(path: &Path) -> Result<Vec<serde_json::Value>, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "failed to open session transcript {}: {error}",
            path.display()
        )
    })?;
    Ok(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect())
}

struct TaskBuilder {
    task: AgentSessionTask,
    activity: ActivityAccumulator,
}

fn build_turn(
    provider: AgentSessionProvider,
    session_id: &str,
    turn_position: usize,
    prompt: &str,
    entries: &[serde_json::Value],
    claude_tasks: &HashMap<String, ClaudeTaskFile>,
    cwd: Option<&str>,
) -> (AgentSessionPromptTurn, ActivityAccumulator) {
    let mut turn_activity = ActivityAccumulator::default();
    let mut tasks = HashMap::<String, TaskBuilder>::new();
    let mut order = Vec::<String>::new();
    let mut active_task = None::<String>;
    let workspace_root = cwd.map(PathBuf::from);

    for entry in entries {
        if let Some(text) = assistant_text(provider, entry) {
            if let Some(task) = active_task.as_ref().and_then(|key| tasks.get_mut(key)) {
                task.task.summary = Some(text);
            }
        }

        match provider {
            AgentSessionProvider::Codex => {
                if let Some(plan) = codex_plan_event(entry) {
                    let mut next_active = None;
                    for (position, item) in plan.plan.into_iter().enumerate() {
                        let key = normalized_task_key(&item.step);
                        if !tasks.contains_key(&key) {
                            order.push(key.clone());
                            tasks.insert(
                                key.clone(),
                                TaskBuilder {
                                    task: AgentSessionTask {
                                        id: task_id("codex", session_id, &key, position),
                                        native_id: None,
                                        subject: item.step.clone(),
                                        description: None,
                                        active_form: None,
                                        status: item.status.into(),
                                        depends_on: Vec::new(),
                                        position,
                                        summary: None,
                                        file_activity: empty_activity(),
                                    },
                                    activity: ActivityAccumulator::default(),
                                },
                            );
                        }
                        if let Some(task) = tasks.get_mut(&key) {
                            task.task.subject = item.step;
                            task.task.status = item.status.into();
                            task.task.position = position;
                        }
                        if matches!(item.status, TaskStatusInput::InProgress) {
                            next_active = Some(key);
                        }
                    }
                    active_task = next_active;
                }
            }
            AgentSessionProvider::Claude => {
                if let Some(event) = claude_task_event(entry, claude_tasks) {
                    let key = event.id.clone();
                    if !tasks.contains_key(&key) {
                        order.push(key.clone());
                        let snapshot = claude_tasks.get(&key);
                        tasks.insert(
                            key.clone(),
                            TaskBuilder {
                                task: AgentSessionTask {
                                    id: format!("claude:{session_id}:{key}"),
                                    native_id: Some(key.clone()),
                                    subject: event
                                        .subject
                                        .clone()
                                        .or_else(|| snapshot.map(|task| task.subject.clone()))
                                        .unwrap_or_else(|| format!("Task {key}")),
                                    description: snapshot.and_then(|task| task.description.clone()),
                                    active_form: snapshot.and_then(|task| task.active_form.clone()),
                                    status: event
                                        .status
                                        .or_else(|| snapshot.map(|task| task.status))
                                        .unwrap_or(AgentSessionTaskStatus::Pending),
                                    depends_on: snapshot
                                        .map(|task| {
                                            task.blocked_by
                                                .iter()
                                                .map(|id| format!("claude:{session_id}:{id}"))
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    position: order.len() - 1,
                                    summary: None,
                                    file_activity: empty_activity(),
                                },
                                activity: ActivityAccumulator::default(),
                            },
                        );
                    }
                    if let Some(status) = event.status {
                        if let Some(task) = tasks.get_mut(&key) {
                            task.task.status = status;
                        }
                        if matches!(status, AgentSessionTaskStatus::InProgress) {
                            active_task = Some(key);
                        } else if active_task.as_deref() == Some(&key) {
                            active_task = None;
                        }
                    }
                }
            }
            AgentSessionProvider::Pi => {}
        }

        collect_entry_activity(
            provider,
            entry,
            workspace_root.as_deref(),
            &mut turn_activity,
        );
        for task in tasks
            .values_mut()
            .filter(|task| !matches!(task.task.status, AgentSessionTaskStatus::Completed))
        {
            collect_entry_activity(
                provider,
                entry,
                workspace_root.as_deref(),
                &mut task.activity,
            );
        }
    }

    let tasks = order
        .into_iter()
        .filter_map(|key| tasks.remove(&key))
        .map(|mut builder| {
            if matches!(provider, AgentSessionProvider::Claude) {
                if let Some(native_id) = builder.task.native_id.as_ref() {
                    if let Some(snapshot) = claude_tasks.get(native_id) {
                        builder.task.status = snapshot.status;
                        builder.task.description = snapshot.description.clone();
                        builder.task.active_form = snapshot.active_form.clone();
                    }
                }
            }
            builder.task.file_activity = finish_file_activity(cwd, builder.activity);
            builder.task
        })
        .collect();
    (
        AgentSessionPromptTurn {
            id: format!("{session_id}:prompt:{turn_position}"),
            prompts: vec![prompt.to_string()],
            summary: final_turn_summary(provider, entries),
            tasks,
            file_activity: finish_file_activity(cwd, turn_activity.clone()),
            started_at_ms: entries.first().map(entry_timestamp_ms).unwrap_or_default(),
        },
        turn_activity,
    )
}

fn collect_entry_activity(
    provider: AgentSessionProvider,
    entry: &serde_json::Value,
    workspace_root: Option<&Path>,
    activity: &mut ActivityAccumulator,
) {
    let timestamp = entry_timestamp_ms(entry);
    match provider {
        AgentSessionProvider::Codex => {
            let Some(payload) = entry.get("payload") else {
                return;
            };
            collect_codex_read_files(payload, workspace_root, timestamp, &mut activity.read_files);
            collect_codex_written_files(payload, workspace_root, timestamp, activity);
        }
        AgentSessionProvider::Claude => collect_tool_call_entry_activity(
            entry,
            ToolSchema::Claude,
            workspace_root,
            timestamp,
            activity,
        ),
        AgentSessionProvider::Pi => collect_tool_call_entry_activity(
            entry,
            ToolSchema::Pi,
            workspace_root,
            timestamp,
            activity,
        ),
    }
}

fn prompt_text(provider: AgentSessionProvider, entry: &serde_json::Value) -> Option<String> {
    let message = provider_message(provider, entry)?;
    (message_role(message) == Some("user"))
        .then(|| message_content(message).and_then(text_content))
        .flatten()
        .map(|text| strip_image_attachment_markers(&text))
        .filter(|text| !text.trim().is_empty())
}

fn assistant_text(provider: AgentSessionProvider, entry: &serde_json::Value) -> Option<String> {
    if matches!(provider, AgentSessionProvider::Codex)
        && entry.get("type").and_then(serde_json::Value::as_str) == Some("event_msg")
        && entry
            .get("payload")
            .and_then(|payload| payload.get("type"))
            .and_then(serde_json::Value::as_str)
            == Some("agent_message")
    {
        return entry
            .get("payload")?
            .get("message")?
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map(str::to_string);
    }
    let message = provider_message(provider, entry)?;
    (message_role(message) == Some("assistant"))
        .then(|| message_content(message).and_then(text_content))
        .flatten()
}

fn final_turn_summary(
    provider: AgentSessionProvider,
    entries: &[serde_json::Value],
) -> Option<String> {
    let mut candidate = None;
    for entry in entries {
        if let Some(text) = assistant_text(provider, entry) {
            candidate = Some(text);
        }
        if entry_has_tool_call(provider, entry) {
            candidate = None;
        }
    }
    candidate
}

fn entry_has_tool_call(provider: AgentSessionProvider, entry: &serde_json::Value) -> bool {
    match provider {
        AgentSessionProvider::Codex => {
            if entry.get("type").and_then(serde_json::Value::as_str) != Some("response_item") {
                return false;
            }
            matches!(
                entry
                    .get("payload")
                    .and_then(|payload| payload.get("type"))
                    .and_then(serde_json::Value::as_str),
                Some("function_call" | "custom_tool_call" | "local_shell_call")
            )
        }
        AgentSessionProvider::Claude | AgentSessionProvider::Pi => entry
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|content| {
                content.iter().any(|item| {
                    matches!(
                        item.get("type").and_then(serde_json::Value::as_str),
                        Some("tool_use" | "toolCall")
                    )
                })
            }),
    }
}

fn provider_message(
    provider: AgentSessionProvider,
    entry: &serde_json::Value,
) -> Option<&serde_json::Value> {
    match provider {
        AgentSessionProvider::Codex => (entry.get("type")?.as_str()? == "response_item")
            .then(|| entry.get("payload"))
            .flatten(),
        AgentSessionProvider::Claude | AgentSessionProvider::Pi => Some(entry),
    }
}

fn message_role(message: &serde_json::Value) -> Option<&str> {
    message
        .get("role")
        .or_else(|| message.get("message")?.get("role"))?
        .as_str()
}

fn message_content(message: &serde_json::Value) -> Option<&serde_json::Value> {
    message
        .get("content")
        .or_else(|| message.get("message")?.get("content"))
}

fn text_content(content: &serde_json::Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return (!text.trim().is_empty()).then(|| text.to_string());
    }
    let text = content
        .as_array()?
        .iter()
        .filter(|item| {
            matches!(
                item.get("type").and_then(serde_json::Value::as_str),
                Some("text" | "input_text" | "output_text")
            )
        })
        .filter_map(|item| item.get("text")?.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.trim().is_empty()).then_some(text)
}

#[derive(Deserialize)]
struct CodexPlan {
    plan: Vec<CodexPlanItem>,
}

#[derive(Deserialize)]
struct CodexPlanItem {
    step: String,
    status: TaskStatusInput,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaskStatusInput {
    Pending,
    InProgress,
    Completed,
}

impl From<TaskStatusInput> for AgentSessionTaskStatus {
    fn from(value: TaskStatusInput) -> Self {
        match value {
            TaskStatusInput::Pending => Self::Pending,
            TaskStatusInput::InProgress => Self::InProgress,
            TaskStatusInput::Completed => Self::Completed,
        }
    }
}

fn codex_plan_event(entry: &serde_json::Value) -> Option<CodexPlan> {
    let payload = entry.get("payload")?;
    match (
        payload.get("type")?.as_str()?,
        payload.get("name")?.as_str()?,
    ) {
        ("custom_tool_call", "exec") => parse_update_plan_call(payload.get("input")?.as_str()?),
        ("function_call", "update_plan") => {
            serde_json::from_str(payload.get("arguments")?.as_str()?).ok()
        }
        _ => None,
    }
}

fn parse_update_plan_call(source: &str) -> Option<CodexPlan> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(source, None)?;
    let mut stack = vec![tree.root_node()];
    let mut latest = None;
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression"
            && node
                .child_by_field_name("function")
                .and_then(|child| child.utf8_text(source.as_bytes()).ok())
                == Some("tools.update_plan")
        {
            let argument = node.child_by_field_name("arguments")?.named_child(0)?;
            latest = js_literal(argument, source.as_bytes())
                .and_then(|value| serde_json::from_value(value).ok())
                .or(latest);
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    latest
}

fn js_literal(node: Node<'_>, source: &[u8]) -> Option<serde_json::Value> {
    match node.kind() {
        "object" => {
            let mut object = serde_json::Map::new();
            let mut cursor = node.walk();
            for pair in node
                .named_children(&mut cursor)
                .filter(|node| node.kind() == "pair")
            {
                let key_node = pair.child_by_field_name("key")?;
                let key = match key_node.kind() {
                    "property_identifier" => key_node.utf8_text(source).ok()?.to_string(),
                    "string" => decode_js_string(key_node.utf8_text(source).ok()?)?,
                    _ => return None,
                };
                object.insert(key, js_literal(pair.child_by_field_name("value")?, source)?);
            }
            Some(object.into())
        }
        "array" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .map(|child| js_literal(child, source))
                .collect::<Option<Vec<_>>>()
                .map(serde_json::Value::Array)
        }
        "string" => decode_js_string(node.utf8_text(source).ok()?).map(Into::into),
        "true" => Some(true.into()),
        "false" => Some(false.into()),
        "null" => Some(serde_json::Value::Null),
        "number" => serde_json::from_str(node.utf8_text(source).ok()?).ok(),
        _ => None,
    }
}

fn decode_js_string(value: &str) -> Option<String> {
    if value.starts_with('"') {
        serde_json::from_str(value).ok()
    } else {
        value
            .strip_prefix('\'')?
            .strip_suffix('\'')
            .map(|inner| inner.replace("\\'", "'").replace("\\\\", "\\"))
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeTaskFile {
    id: String,
    subject: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    active_form: Option<String>,
    status: AgentSessionTaskStatus,
    #[serde(default)]
    blocked_by: Vec<String>,
}

struct ClaudeTaskEvent {
    id: String,
    subject: Option<String>,
    status: Option<AgentSessionTaskStatus>,
}

fn claude_task_event(
    entry: &serde_json::Value,
    snapshots: &HashMap<String, ClaudeTaskFile>,
) -> Option<ClaudeTaskEvent> {
    let content = entry.get("message")?.get("content")?.as_array()?;
    for item in content {
        if item.get("type")?.as_str()? != "tool_use" {
            continue;
        }
        let input = item.get("input")?;
        match item.get("name")?.as_str()? {
            "TaskCreate" => {
                let subject = input.get("subject")?.as_str()?.to_string();
                let id = snapshots
                    .values()
                    .find(|task| task.subject == subject)
                    .map(|task| task.id.clone())
                    .unwrap_or_else(|| normalized_task_key(&subject));
                return Some(ClaudeTaskEvent {
                    id,
                    subject: Some(subject),
                    status: Some(AgentSessionTaskStatus::Pending),
                });
            }
            "TaskUpdate" => {
                let id = input
                    .get("taskId")
                    .or_else(|| input.get("task_id"))?
                    .as_str()?
                    .to_string();
                let status = input
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|status| match status {
                        "pending" => Some(AgentSessionTaskStatus::Pending),
                        "in_progress" => Some(AgentSessionTaskStatus::InProgress),
                        "completed" => Some(AgentSessionTaskStatus::Completed),
                        _ => None,
                    });
                return Some(ClaudeTaskEvent {
                    id,
                    subject: input
                        .get("subject")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    status,
                });
            }
            _ => {}
        }
    }
    None
}

fn read_claude_task_files(
    session_id: &str,
    runtime_home: &Path,
) -> Result<HashMap<String, ClaudeTaskFile>, String> {
    let directory = runtime_home.join("tasks").join(session_id);
    if !directory.is_dir() {
        return Ok(HashMap::new());
    }
    let mut tasks = HashMap::new();
    for entry in fs::read_dir(&directory)
        .map_err(|error| {
            format!(
                "failed to read Claude task store {}: {error}",
                directory.display()
            )
        })?
        .filter_map(Result::ok)
    {
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(contents) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(task) = serde_json::from_str::<ClaudeTaskFile>(&contents) else {
            continue;
        };
        tasks.insert(task.id.clone(), task);
    }
    Ok(tasks)
}

fn normalized_task_key(subject: &str) -> String {
    subject.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn task_id(provider: &str, session_id: &str, key: &str, duplicate: usize) -> String {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{provider}:{session_id}:{:x}:{duplicate}", hasher.finish())
}

fn empty_activity() -> AgentSessionFileActivity {
    AgentSessionFileActivity {
        read_files: Vec::new(),
        edited_files: Vec::new(),
        impacted_files: Vec::new(),
        deleted_files: Vec::new(),
        impacted_relations: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_literal_codex_plan() {
        let plan = parse_update_plan_call(
            r#"const r = await tools.update_plan({plan:[{step:"Inspect API",status:"in_progress"},{step:"Ship",status:"pending"}]}); text(r);"#,
        )
        .expect("parse plan");
        assert_eq!(plan.plan.len(), 2);
        assert_eq!(plan.plan[0].step, "Inspect API");
    }

    #[test]
    fn rejects_dynamic_codex_plan() {
        assert!(parse_update_plan_call("tools.update_plan(buildPlan())").is_none());
    }

    #[test]
    fn filters_image_attachment_markup_from_prompts() {
        let entry = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "<image name=[Image #1]\npath=\"/tmp/clipboard.png\">\n</image>\n\n[Image #1]\n\nKeep this request"
                }]
            }
        });

        assert_eq!(
            prompt_text(AgentSessionProvider::Codex, &entry).as_deref(),
            Some("Keep this request")
        );
    }

    #[test]
    fn distributes_session_impacts_to_matching_activity_segments() {
        let mut activity = AgentSessionFileActivity {
            read_files: Vec::new(),
            edited_files: vec!["/workspace/src/changed.ts".to_string()],
            impacted_files: Vec::new(),
            deleted_files: Vec::new(),
            impacted_relations: Vec::new(),
        };
        let relations = [
            super::super::types::AgentSessionImpactedFileRelation {
                changed_file: "src/changed.ts".to_string(),
                impacted_file: "src/importer.ts".to_string(),
                import_specifier: "./changed".to_string(),
            },
            super::super::types::AgentSessionImpactedFileRelation {
                changed_file: "src/other.ts".to_string(),
                impacted_file: "src/other-importer.ts".to_string(),
                import_specifier: "./other".to_string(),
            },
        ];

        apply_impacted_relations(
            Some("/workspace"),
            &mut activity,
            &relations,
            &[
                "/workspace/src/importer.ts".to_string(),
                "/workspace/src/other-importer.ts".to_string(),
            ],
        );

        assert_eq!(activity.impacted_relations.len(), 1);
        assert_eq!(activity.impacted_files, ["src/importer.ts"]);
        assert_eq!(activity.read_files, ["/workspace/src/importer.ts"]);
    }

    #[test]
    fn breaks_codex_activity_into_prompt_and_task_ranges() {
        let root = std::env::temp_dir().join(format!(
            "coding-agent-va-details-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create test root");
        let transcript_path = root.join("rollout.jsonl");
        let mut transcript = File::create(&transcript_path).expect("create transcript");
        let entries = [
            serde_json::json!({"type":"response_item","timestamp":"2026-07-17T23:59:59Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /tmp/project\n<INSTRUCTIONS>metadata</INSTRUCTIONS>"}]}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-07-18T00:00:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Implement it"}]}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-07-18T00:00:01Z","payload":{"type":"custom_tool_call","name":"exec","call_id":"plan-1","input":"tools.update_plan({plan:[{step:\"Edit UI\",status:\"in_progress\"}]})"}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-07-18T00:00:02Z","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch\n*** Update File: src/App.tsx\n@@\n-old\n+new\n*** End Patch"}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-07-18T00:00:03Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"UI task complete"}]}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-07-18T00:00:04Z","payload":{"type":"custom_tool_call","name":"exec","call_id":"plan-2","input":"tools.update_plan({plan:[{step:\"Edit UI\",status:\"completed\"}]})"}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-07-18T00:00:05Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Everything is done"}]}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-07-18T00:00:06Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"One more change"}]}}),
        ];
        for entry in entries {
            writeln!(transcript, "{entry}").expect("write transcript entry");
        }

        let details = read_session_details(
            AgentSessionProvider::Codex,
            "session",
            &transcript_path,
            &root,
            Some(root.to_str().expect("utf8 root")),
        )
        .expect("read details");

        assert_eq!(details.turns.len(), 2);
        assert_eq!(details.turns[0].prompts, ["Implement it"]);
        assert_eq!(
            details.turns[0].summary.as_deref(),
            Some("Everything is done")
        );
        assert_eq!(details.turns[0].tasks.len(), 1);
        assert_eq!(
            details.turns[0].tasks[0].summary.as_deref(),
            Some("UI task complete")
        );
        assert!(details.turns[0].tasks[0]
            .file_activity
            .edited_files
            .iter()
            .any(|path| path.ends_with("src/App.tsx")));
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn keeps_one_task_across_an_interrupted_prompt_and_requires_a_final_message() {
        let root = std::env::temp_dir().join(format!(
            "coding-agent-va-interrupt-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create test root");
        let transcript_path = root.join("rollout.jsonl");
        let mut transcript = File::create(&transcript_path).expect("create transcript");
        let entries = [
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Start work"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","call_id":"plan-1","input":"tools.update_plan({plan:[{step:\"Shared task\",status:\"in_progress\"}]})"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Intermediate update"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch\n*** Update File: first.ts\n@@\n-a\n+b\n*** End Patch"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Waiting for the command"}],"phase":"commentary"}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"turn_aborted","reason":"interrupted"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Continue after interrupt"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch\n*** Update File: second.ts\n@@\n-a\n+b\n*** End Patch"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","call_id":"plan-2","input":"tools.update_plan({plan:[{step:\"Shared task\",status:\"completed\"}]})"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Final summary"}]}}),
        ];
        for entry in entries {
            writeln!(transcript, "{entry}").expect("write transcript entry");
        }

        let details = read_session_details(
            AgentSessionProvider::Codex,
            "session",
            &transcript_path,
            &root,
            Some(root.to_str().expect("utf8 root")),
        )
        .expect("read details");

        assert_eq!(details.turns.len(), 1);
        assert_eq!(
            details.turns[0].prompts,
            ["Start work", "Continue after interrupt"]
        );
        assert_eq!(details.turns[0].tasks.len(), 1);
        let task = &details.turns[0].tasks[0];
        assert!(matches!(task.status, AgentSessionTaskStatus::Completed));
        assert!(task
            .file_activity
            .edited_files
            .iter()
            .any(|path| path.ends_with("first.ts")));
        assert!(task
            .file_activity
            .edited_files
            .iter()
            .any(|path| path.ends_with("second.ts")));
        assert_eq!(details.turns[0].summary.as_deref(), Some("Final summary"));
        assert!(details.turns[0]
            .file_activity
            .edited_files
            .iter()
            .any(|path| path.ends_with("first.ts")));
        assert!(details.turns[0]
            .file_activity
            .edited_files
            .iter()
            .any(|path| path.ends_with("second.ts")));
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn gives_shared_lifecycle_activity_to_tasks_completed_together() {
        let root = std::env::temp_dir().join(format!(
            "coding-agent-va-multi-complete-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create test root");
        let transcript_path = root.join("rollout.jsonl");
        let mut transcript = File::create(&transcript_path).expect("create transcript");
        let entries = [
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Complete both"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","call_id":"plan-1","input":"tools.update_plan({plan:[{step:\"First task\",status:\"in_progress\"},{step:\"Second task\",status:\"pending\"}]})"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch\n*** Update File: shared.ts\n@@\n-a\n+b\n*** End Patch"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","call_id":"plan-2","input":"tools.update_plan({plan:[{step:\"First task\",status:\"completed\"},{step:\"Second task\",status:\"completed\"}]})"}}),
        ];
        for entry in entries {
            writeln!(transcript, "{entry}").expect("write transcript entry");
        }

        let details = read_session_details(
            AgentSessionProvider::Codex,
            "session",
            &transcript_path,
            &root,
            Some(root.to_str().expect("utf8 root")),
        )
        .expect("read details");

        let tasks = &details.turns[0].tasks;
        assert_eq!(tasks.len(), 2);
        assert!(tasks
            .iter()
            .all(|task| matches!(task.status, AgentSessionTaskStatus::Completed)));
        assert!(tasks.iter().all(|task| task
            .file_activity
            .edited_files
            .iter()
            .any(|path| path.ends_with("shared.ts"))));
        fs::remove_dir_all(root).expect("remove test root");
    }
}
