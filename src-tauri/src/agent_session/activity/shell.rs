use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::agent_session::paths::{
    normalize_absolute_activity_path, normalize_written_activity_path,
};

pub(crate) fn collect_shell_read_files(
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

pub(crate) fn collect_shell_deleted_files(
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

pub(crate) fn insert_activity_path(
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

    Some(normalize_absolute_activity_path(&normalized))
}
