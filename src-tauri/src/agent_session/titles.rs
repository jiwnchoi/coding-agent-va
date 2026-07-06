use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize;

use super::json::{json_str, read_first_json_line};

const SESSION_TITLE_MAX_CHARS: usize = 120;

#[derive(Deserialize)]
struct CodexSessionIndexEntry {
    id: String,
    thread_name: Option<String>,
}

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
    let session_index_path = runtime_home.join("session_index.jsonl");
    let Ok(file) = File::open(session_index_path) else {
        return HashMap::new();
    };

    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<CodexSessionIndexEntry>(&line).ok())
        .filter_map(|entry| entry.thread_name.map(|title| (entry.id, title)))
        .collect()
}

pub(crate) fn extract_codex_session_id(file_name: &str) -> Option<String> {
    file_name
        .strip_prefix("rollout-")?
        .strip_suffix(".jsonl")?
        .rsplit_once('-')
        .map(|(_, session_id)| session_id.to_string())
}

pub(crate) fn read_codex_session_meta_cwd(path: &Path) -> Option<String> {
    let json = read_first_json_line(path)?;
    json_str(&json, &["payload", "cwd"]).map(str::to_string)
}

pub(crate) fn read_codex_first_user_prompt_title(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if json_str(&json, &["type"]) != Some("response_item") {
            continue;
        }

        let Some(payload) = json.get("payload") else {
            continue;
        };
        if json_str(payload, &["type"]) != Some("message")
            || json_str(payload, &["role"]) != Some("user")
        {
            continue;
        }

        let Some(content) = payload.get("content").and_then(|value| value.as_array()) else {
            continue;
        };
        if let Some(title) = first_text_content_title(content, "input_text") {
            return Some(title);
        }
    }

    None
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

fn strip_image_attachment_markers(text: &str) -> String {
    let mut sanitized = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(marker_start) = remaining.find("<image ") {
        sanitized.push_str(&remaining[..marker_start]);

        let after_marker_start = &remaining[marker_start..];
        let Some(marker_end) = after_marker_start.find('>') else {
            sanitized.push_str(after_marker_start);
            return sanitized;
        };

        remaining = &after_marker_start[marker_end + 1..];
    }

    sanitized.push_str(remaining);
    sanitized
}

fn is_metadata_prompt(text: &str) -> bool {
    if text.starts_with("# AGENTS.md instructions") {
        return true;
    }

    let first_token = text.split_whitespace().next().unwrap_or_default();
    first_token.starts_with('<') && first_token.ends_with('>')
}
