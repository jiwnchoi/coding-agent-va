use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::json::{json_str, read_first_json_line};

const SESSION_TITLE_MAX_CHARS: usize = 120;

#[derive(Default)]
pub(crate) struct ClaudeSessionMetadata {
    pub(crate) session_id: Option<String>,
    pub(crate) cwd: Option<String>,
}

#[derive(Clone, Copy)]
enum MessageSchema {
    Claude,
    Pi,
}

pub(crate) fn read_codex_session_titles(runtime_home: &Path) -> HashMap<String, String> {
    let Ok(connection) = Connection::open_with_flags(
        runtime_home.join("state_5.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return HashMap::new();
    };
    let Ok(mut statement) = connection.prepare("SELECT id, title FROM threads WHERE title <> ''")
    else {
        return HashMap::new();
    };
    let Ok(rows) = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return HashMap::new();
    };

    rows.filter_map(Result::ok)
        .map(|(id, title)| (id, normalize_title_whitespace(title)))
        .collect()
}

pub(crate) fn extract_codex_session_id(file_name: &str) -> Option<String> {
    const UUID_CHARS: usize = 36;

    let stem = file_name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let session_id = stem.get(stem.len().checked_sub(UUID_CHARS)?..)?;
    (session_id.len() == UUID_CHARS
        && session_id.chars().enumerate().all(|(index, character)| {
            matches!(index, 8 | 13 | 18 | 23) && character == '-'
                || !matches!(index, 8 | 13 | 18 | 23) && character.is_ascii_hexdigit()
        }))
    .then(|| session_id.to_string())
}

pub(crate) fn read_codex_session_meta_cwd(path: &Path) -> Option<String> {
    let json = read_first_json_line(path)?;
    json_str(&json, &["payload", "cwd"]).map(str::to_string)
}

pub(crate) fn read_pi_session_name(path: &Path) -> Option<String> {
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

pub(crate) fn read_pi_first_user_prompt_title(path: &Path) -> Option<String> {
    read_message_first_user_prompt_title(path, MessageSchema::Pi)
}

pub(crate) fn read_claude_session_metadata(path: &Path) -> Option<ClaudeSessionMetadata> {
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

pub(crate) fn read_claude_ai_title(path: &Path) -> Option<String> {
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

pub(crate) fn read_claude_first_user_prompt_title(path: &Path) -> Option<String> {
    read_message_first_user_prompt_title(path, MessageSchema::Claude)
}

pub(crate) fn normalize_title_whitespace(text: impl AsRef<str>) -> String {
    strip_image_attachment_markers(text.as_ref())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn normalize_title(text: impl AsRef<str>) -> String {
    truncate_title(&normalize_title_whitespace(text), SESSION_TITLE_MAX_CHARS)
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

pub(crate) fn strip_image_attachment_markers(text: &str) -> String {
    let mut sanitized = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(marker_start) = remaining.find("<image name=") {
        sanitized.push_str(&remaining[..marker_start]);

        let after_marker_start = &remaining[marker_start..];
        if let Some(block_end) = after_marker_start.find("</image>") {
            remaining = &after_marker_start[block_end + "</image>".len()..];
            continue;
        }
        let Some(opening_tag_end) = after_marker_start.find('>') else {
            sanitized.push_str(after_marker_start);
            return sanitized;
        };

        remaining = &after_marker_start[opening_tag_end + 1..];
    }

    sanitized.push_str(remaining);
    let mut lines = Vec::new();
    for line in sanitized.lines() {
        let trimmed = line.trim();
        if is_image_reference(trimmed) {
            continue;
        }
        if trimmed.is_empty() {
            if !lines.is_empty() && lines.last().is_some_and(|line: &&str| !line.is_empty()) {
                lines.push("");
            }
            continue;
        }
        lines.push(line);
    }
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines.join("\n")
}

fn is_image_reference(text: &str) -> bool {
    text.strip_prefix("[Image #")
        .and_then(|value| value.strip_suffix(']'))
        .is_some_and(|number| {
            !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
        })
}

pub(crate) fn is_metadata_prompt(text: &str) -> bool {
    if text.starts_with("# AGENTS.md instructions") {
        return true;
    }

    let first_token = text.split_whitespace().next().unwrap_or_default();
    first_token.starts_with('<') && first_token.ends_with('>')
}

#[cfg(test)]
mod tests {
    use super::{extract_codex_session_id, normalize_title, strip_image_attachment_markers};

    #[test]
    fn extracts_full_codex_session_id_from_rollout_name() {
        assert_eq!(
            extract_codex_session_id(
                "rollout-2026-07-17T22-18-24-019f7038-0bdb-73b0-98d9-62198f30edae.jsonl"
            )
            .as_deref(),
            Some("019f7038-0bdb-73b0-98d9-62198f30edae")
        );
    }

    #[test]
    fn normalize_title_removes_newlines_before_truncating() {
        let title = format!("{}\n{}", "a".repeat(119), "b".repeat(10));

        assert_eq!(normalize_title(title), format!("{}…", "a".repeat(119)));
    }

    #[test]
    fn strips_serialized_image_attachments_and_references() {
        let prompt = r#"<image name=[Image #1]
path="/tmp/clipboard.png">

</image>

[Image #1]

Keep this user request.

[ordinary note]"#;

        assert_eq!(
            strip_image_attachment_markers(prompt),
            "Keep this user request.\n\n[ordinary note]"
        );
    }
}
