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
    if json_str(payload, &["type"]) == Some("function_call")
        && matches!(json_str(payload, &["name"]), Some("read" | "read_file"))
    {
        let Some(arguments) = json_str(payload, &["arguments"]) else {
            return;
        };
        let Ok(arguments) = serde_json::from_str::<serde_json::Value>(arguments) else {
            return;
        };
        let Some(file_path) = json_str(&arguments, &["path"])
            .or_else(|| json_str(&arguments, &["file_path"]))
            .or_else(|| json_str(&arguments, &["filePath"]))
        else {
            return;
        };
        insert_activity_path(read_files, file_path, workspace_root, timestamp_ms);
        return;
    }

    let Some(exec_arguments) = extract_exec_command_arguments(payload) else {
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

fn extract_exec_command_arguments(
    payload: &serde_json::Value,
) -> Option<CodexExecCommandArguments> {
    match (json_str(payload, &["type"]), json_str(payload, &["name"])) {
        (Some("function_call"), Some("exec_command")) => json_str(payload, &["arguments"])
            .and_then(|arguments| serde_json::from_str(arguments).ok()),
        (Some("custom_tool_call"), Some("exec")) => {
            json_str(payload, &["input"]).and_then(custom_exec_command_arguments)
        }
        _ => None,
    }
}

fn custom_exec_command_arguments(input: &str) -> Option<CodexExecCommandArguments> {
    let command_call_start = input.find("tools.exec_command(")?;
    let input_after_command_call = &input[command_call_start..];
    let arguments_start = input_after_command_call.find('{')?;
    let arguments = json_object_at(&input_after_command_call[arguments_start..])?;

    serde_json::from_str(arguments).ok()
}

fn json_object_at(input: &str) -> Option<&str> {
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in input.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return input.get(..=index);
                }
            }
            _ => {}
        }
    }

    None
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

#[cfg(test)]
mod tests {
    use super::collect_codex_read_files;
    use crate::agent_session::paths::normalize_absolute_activity_path;
    use std::collections::HashMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn collects_reads_from_custom_exec_tool_calls() {
        let workspace = std::env::temp_dir().join(format!(
            "coding-agent-va-custom-exec-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("current time")
                .as_nanos()
        ));
        fs::create_dir_all(&workspace).expect("create workspace");
        let file_path = workspace.join("target.txt");
        fs::write(&file_path, "content").expect("write target");
        let payload = serde_json::json!({
            "type": "custom_tool_call",
            "name": "exec",
            "input": format!(
                "const result = await tools.exec_command({{\"cmd\":\"rg target.txt\",\"workdir\":\"{}\"}});",
                workspace.display()
            ),
        });
        let mut read_files = HashMap::new();

        collect_codex_read_files(&payload, Some(&workspace), 1, &mut read_files);

        assert!(read_files.contains_key(&normalize_absolute_activity_path(&file_path)));
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }
}
