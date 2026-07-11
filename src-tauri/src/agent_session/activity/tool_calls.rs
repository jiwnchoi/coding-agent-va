use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::shell::{
    collect_shell_deleted_files, collect_shell_edited_files, collect_shell_read_files,
    insert_activity_path,
};
use super::ActivityAccumulator;
use crate::agent_session::json::json_str;
use crate::agent_session::time::entry_timestamp_ms;

#[derive(Clone, Copy)]
pub(crate) enum ToolSchema {
    Claude,
    Pi,
}

pub(crate) fn read_tool_call_file_activity(
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
                collect_shell_edited_files(
                    command,
                    command_root.as_deref(),
                    timestamp_ms,
                    &mut activity.edited_files,
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
