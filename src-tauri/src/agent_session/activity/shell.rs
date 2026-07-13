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

    for token in shell_file_operands(command, false) {
        if let Some(path) = normalize_existing_activity_path(&token, command_root) {
            insert_activity_path(read_files, &path, None, timestamp_ms);
        }
    }
}

pub(crate) fn collect_shell_edited_files(
    command: &str,
    command_root: Option<&Path>,
    timestamp_ms: u64,
    edited_files: &mut HashMap<String, u64>,
) {
    let Some(command_root) = command_root else {
        return;
    };

    for token in shell_file_operands(command, true) {
        if let Some(path) = normalize_existing_activity_path(&token, command_root) {
            insert_activity_path(edited_files, &path, None, timestamp_ms);
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

/// Return operands for commands that consume file contents. Commands such as
/// `find` is intentionally absent: discovering file names is not the same as
/// reading a file's contents. `rg` is included because it searches contents.
fn shell_file_operands(command: &str, edited: bool) -> Vec<String> {
    let tokens = shell_like_tokens(command);
    let Some(command_name) = tokens
        .first()
        .and_then(|token| Path::new(token).file_name().and_then(|name| name.to_str()))
        .map(str::to_owned)
    else {
        return Vec::new();
    };

    if !matches!(
        command_name.as_str(),
        "awk"
            | "bat"
            | "cat"
            | "cut"
            | "head"
            | "less"
            | "more"
            | "nl"
            | "rg"
            | "sed"
            | "sort"
            | "tac"
            | "tail"
            | "tr"
            | "uniq"
            | "wc"
    ) {
        return Vec::new();
    }

    let in_place = tokens_contain_in_place_option(&first_shell_command_tokens(command));
    let mut operands = Vec::new();
    let mut after_options = false;
    for token in tokens.into_iter().skip(1) {
        if token == "--" {
            after_options = true;
            continue;
        }
        if !after_options && token.starts_with('-') {
            continue;
        }
        operands.push(token);
    }

    if command_name != "sed" {
        return if edited { Vec::new() } else { operands };
    }

    if edited {
        if operands.is_empty() {
            return Vec::new();
        }
        // sed -i edits its file operands; without -i sed only reads them.
        // The option may be `-i`, `-i.bak`, or grouped with other flags.
        if in_place {
            operands
        } else {
            Vec::new()
        }
    } else if in_place {
        Vec::new()
    } else {
        operands
    }
}

fn first_shell_command_tokens(command: &str) -> Vec<String> {
    command
        .split([';', '|', '&'])
        .next()
        .map(shell_like_tokens)
        .unwrap_or_default()
}

fn tokens_contain_in_place_option(tokens: &[String]) -> bool {
    tokens
        .iter()
        .skip(1)
        .any(|token| token == "-i" || token.starts_with("-i"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

    fn test_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "coding-agent-va-shell-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos(),
            NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create workspace");
        path
    }

    #[test]
    fn sed_reads_input_files_and_sed_i_edits_them() {
        let workspace = test_workspace();
        let input = workspace.join("input.txt");
        fs::write(&input, "input").expect("write input");
        let mut read_files = HashMap::new();
        let mut edited_files = HashMap::new();

        collect_shell_read_files(
            "sed -n '1,2p' input.txt",
            Some(&workspace),
            1,
            &mut read_files,
        );
        collect_shell_edited_files(
            "sed -i 's/input/output/' input.txt",
            Some(&workspace),
            2,
            &mut edited_files,
        );

        assert!(read_files.contains_key(&normalize_absolute_activity_path(&input)));
        assert!(edited_files.contains_key(&normalize_absolute_activity_path(&input)));
        fs::remove_dir_all(workspace).expect("remove workspace");
    }

    #[test]
    fn sed_read_is_not_marked_as_edit_when_a_later_command_uses_i() {
        let workspace = test_workspace();
        let input = workspace.join("input.txt");
        fs::write(&input, "input").expect("write input");
        let mut edited_files = HashMap::new();

        collect_shell_edited_files(
            "sed -n '1,2p' input.txt && rg -n -i input input.txt",
            Some(&workspace),
            1,
            &mut edited_files,
        );

        assert!(edited_files.is_empty());
        fs::remove_dir_all(workspace).expect("remove workspace");
    }

    #[test]
    fn filename_discovery_commands_do_not_count_as_reads() {
        let workspace = test_workspace();
        let input = workspace.join("input.txt");
        fs::write(&input, "input").expect("write input");
        let mut read_files = HashMap::new();

        collect_shell_read_files("rg input input.txt", Some(&workspace), 1, &mut read_files);
        collect_shell_read_files(
            "find . -name input.txt",
            Some(&workspace),
            1,
            &mut read_files,
        );

        assert!(read_files.contains_key(&normalize_absolute_activity_path(&input)));
        fs::remove_dir_all(workspace).expect("remove workspace");
    }
}
