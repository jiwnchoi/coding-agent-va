use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::json::json_str;
use super::paths::normalize_absolute_activity_path;
use super::types::{AgentSessionFileDiff, AgentSessionProvider};

#[derive(Debug, PartialEq)]
enum SessionEdit {
    Replace {
        before: String,
        after: String,
        replace_all: bool,
    },
    Create(String),
    Delete(Option<String>),
    Write(String),
}

pub(crate) fn read_agent_session_file_diff(
    provider: AgentSessionProvider,
    transcript_path: &Path,
    file_path: &Path,
    cwd: Option<&str>,
    replay_session: bool,
    entry_range: Option<(usize, usize)>,
) -> Result<AgentSessionFileDiff, String> {
    let absolute_file_path = resolve_file_path(file_path, cwd)?;
    let display_path = display_path(&absolute_file_path, cwd);
    let workspace_content = fs::read_to_string(&absolute_file_path).unwrap_or_default();
    let file_missing = !absolute_file_path.is_file();

    if !replay_session {
        return Ok(AgentSessionFileDiff {
            file_path: absolute_file_path.display().to_string(),
            display_path,
            original_content: workspace_content.clone(),
            modified_content: workspace_content,
            diff_base_label: "Workspace".to_string(),
            diff_target_label: "Workspace".to_string(),
            file_missing,
        });
    }

    let edits = read_session_edits(
        provider,
        transcript_path,
        &absolute_file_path,
        cwd,
        entry_range,
    )?;
    if edits.is_empty() {
        return Err(format!(
            "No replayable edits for {display_path} were found in this session."
        ));
    }
    let (original_content, modified_content) =
        replay_snapshots(&edits, &workspace_content).unwrap_or_else(|| snippet_snapshots(&edits));

    Ok(AgentSessionFileDiff {
        file_path: absolute_file_path.display().to_string(),
        display_path,
        original_content,
        modified_content,
        diff_base_label: if entry_range.is_some() {
            "Selection start".to_string()
        } else {
            "Session start".to_string()
        },
        diff_target_label: if entry_range.is_some() {
            "Selection end".to_string()
        } else {
            "Session end".to_string()
        },
        file_missing: matches!(edits.last(), Some(SessionEdit::Delete(_))),
    })
}

fn resolve_file_path(file_path: &Path, cwd: Option<&str>) -> Result<PathBuf, String> {
    if file_path.is_absolute() {
        return Ok(file_path.to_path_buf());
    }
    cwd.map(PathBuf::from)
        .map(|root| root.join(file_path))
        .ok_or_else(|| format!("relative file path requires cwd: {}", file_path.display()))
}

fn display_path(file_path: &Path, cwd: Option<&str>) -> String {
    cwd.and_then(|root| file_path.strip_prefix(root).ok())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| file_path.display().to_string())
}

fn read_session_edits(
    provider: AgentSessionProvider,
    transcript_path: &Path,
    target_path: &Path,
    cwd: Option<&str>,
    entry_range: Option<(usize, usize)>,
) -> Result<Vec<SessionEdit>, String> {
    let file = File::open(transcript_path).map_err(|error| {
        format!(
            "failed to open agent transcript {}: {error}",
            transcript_path.display()
        )
    })?;
    let entries = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(&line).ok())
        .collect::<Vec<_>>();
    let mut edits = Vec::new();
    for (entry_index, entry) in entries.iter().enumerate() {
        if entry_range.is_some_and(|(start, end)| entry_index < start || entry_index >= end) {
            continue;
        }
        match provider {
            AgentSessionProvider::Codex => collect_codex_edits(entry, target_path, cwd, &mut edits),
            AgentSessionProvider::Claude => {
                collect_message_edits(entry, "tool_use", target_path, cwd, &mut edits)
            }
            AgentSessionProvider::Pi => {
                collect_message_edits(entry, "toolCall", target_path, cwd, &mut edits)
            }
        }
    }
    Ok(edits)
}

fn collect_codex_edits(
    entry: &serde_json::Value,
    target_path: &Path,
    cwd: Option<&str>,
    edits: &mut Vec<SessionEdit>,
) {
    if json_str(entry, &["type"]) != Some("response_item") {
        return;
    }
    let Some(payload) = entry.get("payload") else {
        return;
    };
    let name = json_str(payload, &["name"]);
    let patch = match (json_str(payload, &["type"]), name) {
        (Some("custom_tool_call"), Some("apply_patch")) => {
            json_str(payload, &["input"]).map(str::to_string)
        }
        (Some("custom_tool_call"), Some("exec")) => {
            json_str(payload, &["input"]).and_then(custom_exec_apply_patch)
        }
        (Some("function_call"), Some("apply_patch")) => json_str(payload, &["arguments"])
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .and_then(|arguments| {
                json_str(&arguments, &["patch"])
                    .or_else(|| json_str(&arguments, &["input"]))
                    .map(str::to_string)
            }),
        _ => None,
    };
    if let Some(patch) = patch {
        collect_patch_edits(&patch, target_path, cwd, edits);
    }
}

fn collect_message_edits(
    entry: &serde_json::Value,
    item_type: &str,
    target_path: &Path,
    cwd: Option<&str>,
    edits: &mut Vec<SessionEdit>,
) {
    if !matches!(json_str(entry, &["type"]), Some("message" | "assistant")) {
        return;
    }
    let Some(content) = entry
        .get("message")
        .filter(|message| json_str(message, &["role"]) == Some("assistant"))
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    for item in content {
        if json_str(item, &["type"]) != Some(item_type) {
            continue;
        }
        let Some(name) = json_str(item, &["name"]) else {
            continue;
        };
        let Some(arguments) = item.get(if item_type == "tool_use" {
            "input"
        } else {
            "arguments"
        }) else {
            continue;
        };
        collect_structured_edits(name, arguments, target_path, cwd, edits);
    }
}

fn collect_structured_edits(
    tool_name: &str,
    arguments: &serde_json::Value,
    target_path: &Path,
    cwd: Option<&str>,
    edits: &mut Vec<SessionEdit>,
) {
    let normalized_name = tool_name
        .chars()
        .filter(|character| !matches!(character, '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if !matches!(normalized_name.as_str(), "write" | "edit" | "multiedit") {
        return;
    }
    let Some(path) = tool_file_path(arguments) else {
        return;
    };
    if !paths_match(path, target_path, cwd) {
        return;
    }
    if normalized_name == "write" {
        if let Some(content) = json_str(arguments, &["content"]) {
            edits.push(SessionEdit::Write(content.to_string()));
        }
        return;
    }
    if let Some(items) = arguments.get("edits").and_then(serde_json::Value::as_array) {
        for item in items {
            collect_replacement(item, edits);
        }
    } else {
        collect_replacement(arguments, edits);
    }
}

fn collect_replacement(arguments: &serde_json::Value, edits: &mut Vec<SessionEdit>) {
    let before = ["old_string", "oldString", "old_text", "oldText"]
        .into_iter()
        .find_map(|key| json_str(arguments, &[key]));
    let after = ["new_string", "newString", "new_text", "newText"]
        .into_iter()
        .find_map(|key| json_str(arguments, &[key]));
    if let (Some(before), Some(after)) = (before, after) {
        edits.push(SessionEdit::Replace {
            before: before.to_string(),
            after: after.to_string(),
            replace_all: arguments
                .get("replace_all")
                .or_else(|| arguments.get("replaceAll"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        });
    }
}

fn tool_file_path(arguments: &serde_json::Value) -> Option<&str> {
    ["file_path", "filePath", "path"]
        .into_iter()
        .find_map(|key| json_str(arguments, &[key]))
}

fn paths_match(path: &str, target_path: &Path, cwd: Option<&str>) -> bool {
    let path = PathBuf::from(path);
    let absolute = if path.is_absolute() {
        path
    } else if let Some(cwd) = cwd {
        PathBuf::from(cwd).join(path)
    } else {
        return false;
    };
    normalize_absolute_activity_path(&absolute) == normalize_absolute_activity_path(target_path)
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

fn collect_patch_edits(
    patch: &str,
    target_path: &Path,
    cwd: Option<&str>,
    edits: &mut Vec<SessionEdit>,
) {
    let lines = patch.lines().collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let (kind, path) = if let Some(path) = line.strip_prefix("*** Add File: ") {
            ("add", path)
        } else if let Some(path) = line.strip_prefix("*** Update File: ") {
            ("update", path)
        } else if let Some(path) = line.strip_prefix("*** Delete File: ") {
            ("delete", path)
        } else {
            index += 1;
            continue;
        };
        index += 1;
        let start = index;
        while index < lines.len() && !lines[index].starts_with("*** ") {
            index += 1;
        }
        if !paths_match(path, target_path, cwd) {
            continue;
        }
        let body = &lines[start..index];
        match kind {
            "add" => edits.push(SessionEdit::Create(
                body.iter()
                    .filter_map(|line| line.strip_prefix('+'))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
            "delete" => {
                let before = patch_side(body, '-');
                edits.push(SessionEdit::Delete((!before.is_empty()).then_some(before)));
            }
            _ => collect_patch_replacements(body, edits),
        }
    }
}

fn collect_patch_replacements(lines: &[&str], edits: &mut Vec<SessionEdit>) {
    for hunk in lines.split(|line| line.starts_with("@@")) {
        if !hunk.iter().any(|line| line.starts_with(['+', '-'])) {
            continue;
        }
        edits.push(SessionEdit::Replace {
            before: patch_side(hunk, '-'),
            after: patch_side(hunk, '+'),
            replace_all: false,
        });
    }
}

fn patch_side(lines: &[&str], changed_prefix: char) -> String {
    lines
        .iter()
        .filter_map(|line| match line.chars().next() {
            Some('+') | Some('-') if line.starts_with(changed_prefix) => line.get(1..),
            Some('+') | Some('-') => None,
            Some(' ') => line.get(1..),
            _ => Some(*line),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn replay_snapshots(edits: &[SessionEdit], workspace_content: &str) -> Option<(String, String)> {
    let mut original = workspace_content.to_string();
    for edit in edits.iter().rev() {
        original = reverse_edit(edit, &original)?;
    }
    let mut modified = original.clone();
    for edit in edits {
        modified = apply_edit(edit, &modified)?;
    }
    Some((original, modified))
}

fn reverse_edit(edit: &SessionEdit, content: &str) -> Option<String> {
    match edit {
        SessionEdit::Replace {
            before,
            after,
            replace_all,
        } => replace(content, after, before, *replace_all),
        SessionEdit::Create(after) if content == after => Some(String::new()),
        SessionEdit::Delete(Some(before)) if content.is_empty() => Some(before.clone()),
        _ => None,
    }
}

fn apply_edit(edit: &SessionEdit, content: &str) -> Option<String> {
    match edit {
        SessionEdit::Replace {
            before,
            after,
            replace_all,
        } => replace(content, before, after, *replace_all),
        SessionEdit::Create(after) if content.is_empty() => Some(after.clone()),
        SessionEdit::Delete(_) => Some(String::new()),
        SessionEdit::Write(after) => Some(after.clone()),
        _ => None,
    }
}

fn replace(content: &str, from: &str, to: &str, replace_all: bool) -> Option<String> {
    if from.is_empty() {
        return None;
    }
    if replace_all {
        content.contains(from).then(|| content.replace(from, to))
    } else {
        let index = content.find(from)?;
        let mut result = content.to_string();
        result.replace_range(index..index + from.len(), to);
        Some(result)
    }
}

fn snippet_snapshots(edits: &[SessionEdit]) -> (String, String) {
    let mut before = Vec::new();
    let mut after = Vec::new();
    for edit in edits {
        match edit {
            SessionEdit::Replace {
                before: old,
                after: new,
                ..
            } => {
                before.push(old.as_str());
                after.push(new.as_str());
            }
            SessionEdit::Create(content) | SessionEdit::Write(content) => after.push(content),
            SessionEdit::Delete(Some(content)) => before.push(content),
            SessionEdit::Delete(None) => {}
        }
    }
    (before.join("\n"), after.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{
        collect_message_edits, collect_patch_edits, read_session_edits, replay_snapshots,
        SessionEdit,
    };
    use crate::agent_session::types::AgentSessionProvider;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn replays_multiple_session_replacements_from_workspace_snapshot() {
        let edits = vec![
            SessionEdit::Replace {
                before: "one".to_string(),
                after: "two".to_string(),
                replace_all: false,
            },
            SessionEdit::Replace {
                before: "two apples".to_string(),
                after: "three apples".to_string(),
                replace_all: false,
            },
        ];

        assert_eq!(
            replay_snapshots(&edits, "three apples\n"),
            Some(("one apples\n".to_string(), "three apples\n".to_string()))
        );
    }

    #[test]
    fn extracts_target_file_hunks_from_apply_patch() {
        let mut edits = Vec::new();
        collect_patch_edits(
            "*** Begin Patch\n*** Update File: src/app.ts\n@@\n const value =\n-old\n+new\n*** End Patch",
            Path::new("/workspace/src/app.ts"),
            Some("/workspace"),
            &mut edits,
        );

        assert_eq!(
            edits,
            vec![SessionEdit::Replace {
                before: "const value =\nold".to_string(),
                after: "const value =\nnew".to_string(),
                replace_all: false,
            }]
        );
    }

    #[test]
    fn extracts_claude_and_pi_structured_edits() {
        let target = Path::new("/workspace/src/app.ts");
        let mut edits = Vec::new();
        let claude_entry = serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "name": "Edit",
                    "input": {
                        "file_path": "src/app.ts",
                        "old_string": "before",
                        "new_string": "after"
                    }
                }]
            }
        });
        let pi_entry = serde_json::json!({
            "type": "message",
            "message": {
                "role": "assistant",
                "content": [{
                    "type": "toolCall",
                    "name": "write",
                    "arguments": {
                        "path": "src/app.ts",
                        "content": "final"
                    }
                }]
            }
        });

        collect_message_edits(
            &claude_entry,
            "tool_use",
            target,
            Some("/workspace"),
            &mut edits,
        );
        collect_message_edits(
            &pi_entry,
            "toolCall",
            target,
            Some("/workspace"),
            &mut edits,
        );

        assert_eq!(
            edits,
            vec![
                SessionEdit::Replace {
                    before: "before".to_string(),
                    after: "after".to_string(),
                    replace_all: false,
                },
                SessionEdit::Write("final".to_string()),
            ]
        );
    }

    #[test]
    fn limits_replayed_edits_to_the_selected_entry_range() {
        let root = std::env::temp_dir().join(format!(
            "coding-agent-va-session-diff-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).expect("create test root");
        let transcript_path = root.join("rollout.jsonl");
        let mut transcript = File::create(&transcript_path).expect("create transcript");
        for patch in [
            "*** Begin Patch\n*** Update File: src/app.ts\n@@\n-old\n+first\n*** End Patch",
            "*** Begin Patch\n*** Update File: src/app.ts\n@@\n-first\n+second\n*** End Patch",
        ] {
            let entry = serde_json::json!({
                "type": "response_item",
                "payload": { "type": "custom_tool_call", "name": "apply_patch", "input": patch }
            });
            writeln!(transcript, "{entry}").expect("write transcript entry");
        }

        let edits = read_session_edits(
            AgentSessionProvider::Codex,
            &transcript_path,
            &root.join("src/app.ts"),
            root.to_str(),
            Some((0, 1)),
        )
        .expect("read scoped edits");

        assert_eq!(
            edits,
            vec![SessionEdit::Replace {
                before: "old".to_string(),
                after: "first".to_string(),
                replace_all: false,
            }]
        );
        fs::remove_dir_all(root).expect("remove test root");
    }
}
