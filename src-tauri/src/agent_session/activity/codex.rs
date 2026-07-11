use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::shell::{
    collect_shell_deleted_files, collect_shell_edited_files, collect_shell_read_files,
    insert_activity_path,
};
use crate::agent_session::activity::ActivityAccumulator;
use crate::agent_session::json::json_str;

#[derive(Deserialize)]
pub(crate) struct CodexRolloutEnvelope {
    #[serde(rename = "type")]
    pub(crate) entry_type: String,
    pub(crate) payload: serde_json::Value,
}

#[derive(Deserialize)]
struct CodexExecCommandArguments {
    #[serde(alias = "command", deserialize_with = "deserialize_command")]
    cmd: String,
    workdir: Option<String>,
}

fn deserialize_command<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(command) => Ok(command),
        serde_json::Value::Array(arguments) => arguments
            .last()
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| serde::de::Error::custom("command array has no script")),
        _ => Err(serde::de::Error::custom("command is not a string or array")),
    }
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

    for exec_arguments in extract_exec_command_arguments(payload) {
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
}

fn extract_exec_command_arguments(payload: &serde_json::Value) -> Vec<CodexExecCommandArguments> {
    match (json_str(payload, &["type"]), json_str(payload, &["name"])) {
        (Some("function_call"), Some("exec_command" | "shell" | "shell_command")) => {
            json_str(payload, &["arguments"])
                .and_then(|arguments| serde_json::from_str(arguments).ok())
                .into_iter()
                .collect()
        }
        (Some("custom_tool_call"), Some("exec")) => json_str(payload, &["input"])
            .map(custom_exec_command_arguments)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn custom_exec_command_arguments(input: &str) -> Vec<CodexExecCommandArguments> {
    let mut arguments = Vec::new();
    let mut remaining = input;

    while let Some(command_call_start) = remaining.find("tools.exec_command(") {
        let input_after_command_call = &remaining[command_call_start..];
        let Some(arguments_start) = input_after_command_call.find('{') else {
            break;
        };
        let arguments_input = &input_after_command_call[arguments_start..];
        let Some(arguments_json) = json_object_at(arguments_input) else {
            break;
        };
        if let Some(command_arguments) = parse_exec_command_arguments(arguments_json) {
            arguments.push(command_arguments);
        }
        remaining = &arguments_input[arguments_json.len()..];
    }

    arguments
}

fn parse_exec_command_arguments(input: &str) -> Option<CodexExecCommandArguments> {
    serde_json::from_str(input).ok().or_else(|| {
        let json = quote_javascript_object_keys(input)?;
        serde_json::from_str(&json).ok()
    })
}

/// Codex records the source passed to the JavaScript `exec` orchestration tool.
/// Object literals in that source commonly use unquoted keys (`{cmd: "..."}`),
/// so make those keys valid JSON before deserializing the arguments.
fn quote_javascript_object_keys(input: &str) -> Option<String> {
    let mut output = String::with_capacity(input.len() + 8);
    let mut characters = input.char_indices().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut expects_key = false;

    while let Some((index, character)) = characters.next() {
        if in_string {
            output.push(character);
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
            '"' => {
                in_string = true;
                output.push(character);
                expects_key = false;
            }
            '{' | ',' => {
                output.push(character);
                expects_key = true;
            }
            character if expects_key && character.is_ascii_whitespace() => output.push(character),
            character if expects_key && (character.is_ascii_alphabetic() || character == '_') => {
                let start = index;
                let mut end = index + character.len_utf8();
                while let Some(&(next_index, next_character)) = characters.peek() {
                    if next_character.is_ascii_alphanumeric() || next_character == '_' {
                        characters.next();
                        end = next_index + next_character.len_utf8();
                    } else {
                        break;
                    }
                }
                if characters.peek().is_some_and(|(_, next)| *next == ':') {
                    output.push('"');
                    output.push_str(input.get(start..end)?);
                    output.push('"');
                } else {
                    output.push_str(input.get(start..end)?);
                }
                expects_key = false;
            }
            _ => {
                output.push(character);
                if !character.is_ascii_whitespace() {
                    expects_key = false;
                }
            }
        }
    }

    (!in_string).then_some(output)
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
    workspace_root: Option<&Path>,
    timestamp_ms: u64,
    activity: &mut ActivityAccumulator,
) {
    if json_str(payload, &["type"]) == Some("patch_apply_end") {
        if let Some(changes) = payload.get("changes").and_then(|value| value.as_object()) {
            for (path, change) in changes {
                match json_str(change, &["type"]) {
                    Some("delete") => insert_activity_path(
                        &mut activity.deleted_files,
                        path,
                        workspace_root,
                        timestamp_ms,
                    ),
                    Some("add" | "update" | "move") => {
                        insert_activity_path(
                            &mut activity.edited_files,
                            path,
                            workspace_root,
                            timestamp_ms,
                        );
                    }
                    _ => {}
                }
            }
        }
        return;
    }

    if matches!(
        (json_str(payload, &["type"]), json_str(payload, &["name"])),
        (Some("custom_tool_call"), Some("apply_patch"))
    ) {
        if let Some(patch) = json_str(payload, &["input"]) {
            collect_apply_patch_files(patch, workspace_root, timestamp_ms, activity);
        }
        return;
    }

    if matches!(
        (json_str(payload, &["type"]), json_str(payload, &["name"])),
        (Some("custom_tool_call"), Some("exec"))
    ) {
        if let Some(patch) = json_str(payload, &["input"]).and_then(custom_exec_apply_patch) {
            collect_apply_patch_files(&patch, workspace_root, timestamp_ms, activity);
        }
    }

    for exec_arguments in extract_exec_command_arguments(payload) {
        let command_root = exec_arguments
            .workdir
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| workspace_root.map(Path::to_path_buf));
        collect_shell_edited_files(
            &exec_arguments.cmd,
            command_root.as_deref(),
            timestamp_ms,
            &mut activity.edited_files,
        );
        collect_shell_deleted_files(
            &exec_arguments.cmd,
            command_root.as_deref(),
            timestamp_ms,
            &mut activity.deleted_files,
        );
    }
}

fn custom_exec_apply_patch(input: &str) -> Option<String> {
    let call = input.find("tools.apply_patch(")?;
    let argument = input[call + "tools.apply_patch(".len()..]
        .split_once(')')?
        .0
        .trim();
    let encoded = if argument.starts_with('"') {
        argument
    } else {
        let assignment = format!("const {argument} = ");
        let start = input[..call].rfind(&assignment)? + assignment.len();
        input[start..].trim_start()
    };
    let encoded = json_string_at(encoded)?;
    serde_json::from_str(encoded).ok()
}

fn json_string_at(input: &str) -> Option<&str> {
    if !input.starts_with('"') {
        return None;
    }
    let mut escaped = false;
    for (index, character) in input.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return input.get(..=index);
        }
    }
    None
}

fn collect_apply_patch_files(
    patch: &str,
    workspace_root: Option<&Path>,
    timestamp_ms: u64,
    activity: &mut ActivityAccumulator,
) {
    let mut current_path: Option<&str> = None;
    let mut added_lines = Vec::new();
    let flush_fragment =
        |path: Option<&str>, lines: &mut Vec<&str>, activity: &mut ActivityAccumulator| {
            let Some(path) = path else {
                return;
            };
            if !lines.is_empty() {
                let fragment = lines.join("\n");
                activity.record_edit_fragment(path, workspace_root, Some(&fragment));
                lines.clear();
            } else {
                activity.record_edit_fragment(path, workspace_root, None);
            }
        };
    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            flush_fragment(current_path, &mut added_lines, activity);
            current_path = Some(path);
            insert_activity_path(
                &mut activity.edited_files,
                path,
                workspace_root,
                timestamp_ms,
            );
            activity.record_edit_fragment(path, workspace_root, None);
            continue;
        }
        let (target, path) = if let Some(path) = line.strip_prefix("*** Delete File: ") {
            flush_fragment(current_path, &mut added_lines, activity);
            current_path = None;
            (&mut activity.deleted_files, path)
        } else if let Some(path) = line
            .strip_prefix("*** Update File: ")
            .or_else(|| line.strip_prefix("*** Move to: "))
        {
            flush_fragment(current_path, &mut added_lines, activity);
            current_path = Some(path);
            (&mut activity.edited_files, path)
        } else {
            if current_path.is_some() && line.starts_with('+') && !line.starts_with("+++") {
                added_lines.push(&line[1..]);
            }
            continue;
        };
        insert_activity_path(target, path, workspace_root, timestamp_ms);
    }
    flush_fragment(current_path, &mut added_lines, activity);
}

#[cfg(test)]
mod tests {
    use super::{collect_codex_read_files, collect_codex_written_files};
    use crate::agent_session::activity::ActivityAccumulator;
    use crate::agent_session::paths::normalize_absolute_activity_path;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

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

    #[test]
    fn collects_reads_from_every_parallel_custom_exec_command() {
        let workspace = test_workspace();
        let first_file = workspace.join("first.ts");
        let second_file = workspace.join("second.ts");
        let third_file = workspace.join("third.json");
        for file_path in [&first_file, &second_file, &third_file] {
            fs::write(file_path, "content").expect("write target");
        }
        let payload = serde_json::json!({
            "type": "custom_tool_call",
            "name": "exec",
            "input": format!(
                "const results = await Promise.all([\n  tools.exec_command({{\"cmd\":\"sed -n '1,20p' first.ts\",\"workdir\":\"{}\"}}),\n  tools.exec_command({{\"cmd\":\"sed -n '1,20p' second.ts && sed -n '1,20p' third.json\",\"workdir\":\"{}\"}})\n]);",
                workspace.display(),
                workspace.display()
            ),
        });
        let mut read_files = HashMap::new();

        collect_codex_read_files(&payload, Some(&workspace), 1, &mut read_files);

        for file_path in [first_file, second_file, third_file] {
            assert!(read_files.contains_key(&normalize_absolute_activity_path(&file_path)));
        }
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }

    #[test]
    fn collects_reads_from_current_javascript_object_arguments() {
        let workspace = test_workspace();
        let view = workspace.join("src/SessionContextGraphView.tsx");
        let graph = workspace.join("src/buildContextGraph.ts");
        fs::create_dir_all(view.parent().expect("source parent")).expect("create source directory");
        fs::write(&view, "content").expect("write view");
        fs::write(&graph, "content").expect("write graph");
        let payload = serde_json::json!({
            "type": "custom_tool_call",
            "name": "exec",
            "input": format!(
                "const r = await tools.exec_command({{cmd:\"sed -n '1,380p' src/SessionContextGraphView.tsx && sed -n '1,380p' src/buildContextGraph.ts\",workdir:\"{}\",yield_time_ms:10000,max_output_tokens:50000}}); text(r.output);",
                workspace.display()
            ),
        });
        let mut read_files = HashMap::new();

        collect_codex_read_files(&payload, Some(&workspace), 1, &mut read_files);

        for file_path in [view, graph] {
            assert!(read_files.contains_key(&normalize_absolute_activity_path(&file_path)));
        }
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }

    #[test]
    fn collects_writes_from_current_apply_patch_calls() {
        let workspace = test_workspace();
        fs::create_dir_all(workspace.join("src")).expect("create source directory");
        let payload = serde_json::json!({
            "type": "custom_tool_call",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** Update File: src/lib.rs\n+fn changed() {}\n*** Move to: src/main.rs\n*** Delete File: old.rs\n*** End Patch",
        });
        let mut activity = ActivityAccumulator::default();

        collect_codex_written_files(&payload, Some(&workspace), 7, &mut activity);

        assert!(activity
            .edited_files
            .contains_key(&normalize_absolute_activity_path(
                &workspace.join("src/lib.rs")
            )));
        assert!(activity
            .edited_files
            .contains_key(&normalize_absolute_activity_path(
                &workspace.join("src/main.rs")
            )));
        assert!(activity
            .deleted_files
            .contains_key(&normalize_absolute_activity_path(&workspace.join("old.rs"))));
        assert_eq!(
            activity.edit_fragments
                [&normalize_absolute_activity_path(&workspace.join("src/lib.rs"))],
            vec!["fn changed() {}"]
        );
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }

    #[test]
    fn collects_reads_from_legacy_shell_array_calls() {
        let workspace = test_workspace();
        let file_path = workspace.join("target.txt");
        fs::write(&file_path, "content").expect("write target");
        let payload = serde_json::json!({
            "type": "function_call",
            "name": "shell",
            "arguments": serde_json::json!({
                "command": ["bash", "-lc", "sed -n '1p' target.txt"],
                "workdir": workspace,
            }).to_string(),
        });
        let mut read_files = HashMap::new();

        collect_codex_read_files(&payload, Some(&workspace), 1, &mut read_files);

        assert!(read_files.contains_key(&normalize_absolute_activity_path(&file_path)));
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }

    #[test]
    fn collects_writes_from_current_custom_exec_wrapper() {
        let workspace = test_workspace();
        let payload = serde_json::json!({
            "type": "custom_tool_call",
            "name": "exec",
            "input": "const patch = \"*** Begin Patch\\n*** Add File: src/new.rs\\n*** End Patch\"; text(await tools.apply_patch(patch));",
        });
        let mut activity = ActivityAccumulator::default();

        collect_codex_written_files(&payload, Some(&workspace), 9, &mut activity);

        assert_eq!(activity.edited_files.len(), 1);
        assert_eq!(activity.deleted_files.len(), 0);
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }

    fn test_workspace() -> PathBuf {
        let workspace = std::env::temp_dir().join(format!(
            "coding-agent-va-codex-test-{}-{}-{}",
            std::process::id(),
            TEST_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("current time")
                .as_nanos()
        ));
        fs::create_dir_all(&workspace).expect("create workspace");
        workspace
    }
}
