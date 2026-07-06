use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::shell::{collect_shell_read_files, insert_activity_path};
use crate::agent_session::json::json_str;

#[derive(Deserialize)]
pub(crate) struct CodexRolloutEnvelope {
    #[serde(rename = "type")]
    pub(crate) entry_type: String,
    pub(crate) payload: serde_json::Value,
}

#[derive(Deserialize)]
struct CodexExecCommandArguments {
    cmd: String,
    workdir: Option<String>,
}

pub(crate) fn collect_codex_read_files(
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

pub(crate) fn collect_codex_written_files(
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
