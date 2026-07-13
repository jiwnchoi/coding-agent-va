use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::{
    AgentSessionNodeDescriptionRequest, AgentSessionNodeDescriptionResponse,
    AgentSessionNodeDescriptionStreamEvent, AgentSessionProvider, DescriptionGraphNode,
};
use crate::app_config::DescriptionReasoning;

mod codex_api;

const GENERAL_SYSTEM_PROMPT: &str =
    include_str!("../../../configs/node-description-system-prompt.md");
const EDITED_SYSTEM_PROMPT: &str =
    include_str!("../../../configs/node-description-edited-system-prompt.md");
const IMPACTED_SYSTEM_PROMPT: &str =
    include_str!("../../../configs/node-description-impacted-system-prompt.md");
const MAX_RELATED_NODES: usize = 10;
const MAX_SOURCE_CHARS_PER_FILE: usize = 8_000;
const MAX_SOURCE_CHARS_TOTAL: usize = 48_000;
const MAX_DIFF_CHARS: usize = 20_000;
const MAX_SESSION_EDIT_CHARS: usize = 20_000;
const MAX_CACHED_DESCRIPTIONS: usize = 64;

#[derive(Clone, Default)]
pub(crate) struct NodeDescriptionCacheState {
    entries: Arc<Mutex<VecDeque<(u64, AgentSessionNodeDescriptionResponse)>>>,
}

impl NodeDescriptionCacheState {
    fn get(&self, key: u64) -> Result<Option<AgentSessionNodeDescriptionResponse>, String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "failed to lock node description cache".to_string())?;
        let Some(index) = entries
            .iter()
            .position(|(cached_key, _)| *cached_key == key)
        else {
            return Ok(None);
        };
        let entry = entries
            .remove(index)
            .ok_or_else(|| "cached node description disappeared".to_string())?;
        let response = entry.1.clone();
        entries.push_back(entry);
        Ok(Some(response))
    }

    fn insert(
        &self,
        key: u64,
        response: AgentSessionNodeDescriptionResponse,
    ) -> Result<(), String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "failed to lock node description cache".to_string())?;
        entries.retain(|(cached_key, _)| *cached_key != key);
        entries.push_back((key, response));
        while entries.len() > MAX_CACHED_DESCRIPTIONS {
            entries.pop_front();
        }
        Ok(())
    }
}

pub(crate) fn describe_session_node(
    request: AgentSessionNodeDescriptionRequest,
    cache: &NodeDescriptionCacheState,
    on_event: impl Fn(AgentSessionNodeDescriptionStreamEvent) -> Result<(), String>,
) -> Result<AgentSessionNodeDescriptionResponse, String> {
    let workspace_root = canonical_workspace_root(&request.cwd)?;
    let prompt = build_description_prompt(&request, &workspace_root)?;
    let cache_key = description_cache_key(&request, &prompt);
    if let Some(response) = cache.get(cache_key)? {
        on_event(AgentSessionNodeDescriptionStreamEvent::Started {
            provider_label: response.provider_label.clone(),
            cached: true,
        })?;
        on_event(AgentSessionNodeDescriptionStreamEvent::Chunk {
            text: response.description.clone(),
        })?;
        return Ok(response);
    }
    on_event(AgentSessionNodeDescriptionStreamEvent::Started {
        provider_label: request.provider.label().to_string(),
        cached: false,
    })?;
    let emit_chunk = |text: &str| {
        on_event(AgentSessionNodeDescriptionStreamEvent::Chunk {
            text: text.to_string(),
        })
    };
    let description = match request.provider {
        AgentSessionProvider::Codex => codex_api::run(
            Path::new(&request.runtime_home),
            Path::new(&request.transcript_path),
            request.model.trim(),
            reasoning_value(request.reasoning),
            system_prompt(&request).trim(),
            &prompt,
            emit_chunk,
        ),
        AgentSessionProvider::Claude | AgentSessionProvider::Pi => run_cli(
            cli_invocation(&request),
            &workspace_root,
            &prompt,
            emit_chunk,
        ),
    }?;

    if description.trim().is_empty() {
        return Err(format!(
            "{} returned an empty description",
            request.provider.label()
        ));
    }

    let response = AgentSessionNodeDescriptionResponse {
        description: description.trim().to_string(),
        provider_label: request.provider.label().to_string(),
    };
    cache.insert(cache_key, response.clone())?;
    Ok(response)
}

fn description_cache_key(request: &AgentSessionNodeDescriptionRequest, prompt: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    request.provider.hash(&mut hasher);
    request.provider_session_id.hash(&mut hasher);
    request.model.hash(&mut hasher);
    reasoning_value(request.reasoning).hash(&mut hasher);
    system_prompt(request).hash(&mut hasher);
    if let Ok(metadata) = fs::metadata(&request.transcript_path) {
        metadata.len().hash(&mut hasher);
        metadata.modified().ok().hash(&mut hasher);
    }
    prompt.hash(&mut hasher);
    hasher.finish()
}

struct CliInvocation {
    program: &'static str,
    args: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
    output_format: CliOutputFormat,
    temporary_session_dir: Option<PathBuf>,
}

#[derive(Clone, Copy)]
enum CliOutputFormat {
    PlainText,
    ClaudeStreamJson,
}

struct TemporarySessionDir(Option<PathBuf>);

impl TemporarySessionDir {
    fn create(path: Option<&Path>, program: &str) -> Result<Self, String> {
        if let Some(path) = path {
            fs::create_dir_all(path).map_err(|error| {
                format!("failed to create temporary {program} session directory: {error}")
            })?;
        }
        Ok(Self(path.map(Path::to_path_buf)))
    }
}

impl Drop for TemporarySessionDir {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn cli_invocation(request: &AgentSessionNodeDescriptionRequest) -> CliInvocation {
    match request.provider {
        AgentSessionProvider::Codex => unreachable!("Codex descriptions use the Responses API"),
        AgentSessionProvider::Claude => {
            let mut args = vec![
                "--print".into(),
                "--resume".into(),
                request.provider_session_id.clone().into(),
                "--fork-session".into(),
                "--no-session-persistence".into(),
                "--tools".into(),
                "".into(),
                "--append-system-prompt".into(),
                system_prompt(request).trim().into(),
                "--output-format".into(),
                "stream-json".into(),
                "--include-partial-messages".into(),
                "--verbose".into(),
                "--settings".into(),
                format!(
                    r#"{{"alwaysThinkingEnabled":{}}}"#,
                    request.reasoning != DescriptionReasoning::None
                )
                .into(),
            ];
            append_model_args(&mut args, &request.model, "--model");
            if request.reasoning != DescriptionReasoning::None {
                args.extend(["--effort".into(), claude_effort(request.reasoning).into()]);
            }
            CliInvocation {
                program: "claude",
                args,
                environment: vec![(
                    "CLAUDE_CONFIG_DIR".into(),
                    request.runtime_home.clone().into(),
                )],
                output_format: CliOutputFormat::ClaudeStreamJson,
                temporary_session_dir: None,
            }
        }
        AgentSessionProvider::Pi => {
            let temporary_session_dir = temporary_pi_session_dir();
            let mut args = vec![
                "--print".into(),
                "--fork".into(),
                request.transcript_path.clone().into(),
                "--session-dir".into(),
                temporary_session_dir.clone().into(),
                "--no-tools".into(),
                "--no-extensions".into(),
                "--no-skills".into(),
                "--no-context-files".into(),
                "--system-prompt".into(),
                system_prompt(request).trim().into(),
                "--thinking".into(),
                pi_thinking(request.reasoning).into(),
            ];
            append_model_args(&mut args, &request.model, "--model");
            CliInvocation {
                program: "pi",
                args,
                environment: vec![(
                    "PI_CODING_AGENT_DIR".into(),
                    request.runtime_home.clone().into(),
                )],
                output_format: CliOutputFormat::PlainText,
                temporary_session_dir: Some(temporary_session_dir),
            }
        }
    }
}

fn system_prompt(request: &AgentSessionNodeDescriptionRequest) -> &'static str {
    if request
        .clicked_node
        .activities
        .iter()
        .any(|activity| activity == "edited")
    {
        EDITED_SYSTEM_PROMPT
    } else if request
        .clicked_node
        .activities
        .iter()
        .any(|activity| activity == "impacted")
    {
        IMPACTED_SYSTEM_PROMPT
    } else {
        GENERAL_SYSTEM_PROMPT
    }
}

fn append_model_args(args: &mut Vec<OsString>, model: &str, flag: &str) {
    if !model.trim().is_empty() {
        args.extend([flag.into(), model.trim().into()]);
    }
}

fn reasoning_value(reasoning: DescriptionReasoning) -> &'static str {
    match reasoning {
        DescriptionReasoning::None => "none",
        DescriptionReasoning::Minimal => "minimal",
        DescriptionReasoning::Low => "low",
        DescriptionReasoning::Medium => "medium",
        DescriptionReasoning::High => "high",
        DescriptionReasoning::Xhigh => "xhigh",
        DescriptionReasoning::Max => "max",
    }
}

fn claude_effort(reasoning: DescriptionReasoning) -> &'static str {
    match reasoning {
        DescriptionReasoning::None | DescriptionReasoning::Minimal | DescriptionReasoning::Low => {
            "low"
        }
        DescriptionReasoning::Medium => "medium",
        DescriptionReasoning::High => "high",
        DescriptionReasoning::Xhigh => "xhigh",
        DescriptionReasoning::Max => "max",
    }
}

fn pi_thinking(reasoning: DescriptionReasoning) -> &'static str {
    match reasoning {
        DescriptionReasoning::None => "off",
        _ => reasoning_value(reasoning),
    }
}

fn run_cli(
    invocation: CliInvocation,
    cwd: &Path,
    prompt: &str,
    on_chunk: impl Fn(&str) -> Result<(), String>,
) -> Result<String, String> {
    let _temporary_session_dir = TemporarySessionDir::create(
        invocation.temporary_session_dir.as_deref(),
        invocation.program,
    )?;
    let mut command = Command::new(invocation.program);
    command.args(&invocation.args).current_dir(cwd);
    for (key, value) in &invocation.environment {
        command.env(key, value);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "failed to start {} CLI; ensure it is installed and available in PATH: {error}",
                invocation.program
            )
        })?;

    child
        .stdin
        .take()
        .ok_or_else(|| format!("failed to open {} stdin", invocation.program))?
        .write_all(prompt.as_bytes())
        .map_err(|error| format!("failed to send context to {}: {error}", invocation.program))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to open {} stdout", invocation.program))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to open {} stderr", invocation.program))?;
    let stderr_reader = std::thread::spawn(move || {
        let mut stderr_text = String::new();
        BufReader::new(stderr).read_to_string(&mut stderr_text)?;
        Ok::<_, std::io::Error>(stderr_text)
    });
    let description = match invocation.output_format {
        CliOutputFormat::PlainText => stream_plain_text(stdout, on_chunk)?,
        CliOutputFormat::ClaudeStreamJson => stream_claude_json(stdout, on_chunk)?,
    };
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for {}: {error}", invocation.program))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| format!("{} stderr reader panicked", invocation.program))?
        .map_err(|error| format!("failed to read {} stderr: {error}", invocation.program))?;
    if !status.success() {
        return Err(format!(
            "{} CLI failed: {}",
            invocation.program,
            stderr.trim().chars().take(2_000).collect::<String>()
        ));
    }

    Ok(description)
}

fn stream_plain_text(
    stdout: impl Read,
    on_chunk: impl Fn(&str) -> Result<(), String>,
) -> Result<String, String> {
    let mut reader = BufReader::new(stdout);
    let mut buffer = [0_u8; 1024];
    let mut pending = Vec::new();
    let mut description = String::new();
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to read description output: {error}"))?;
        if count == 0 {
            break;
        }
        pending.extend_from_slice(&buffer[..count]);
        let valid_length = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(error) => {
                return Err(format!(
                    "description CLI returned non-UTF-8 output: {error}"
                ));
            }
        };
        if valid_length > 0 {
            let chunk = std::str::from_utf8(&pending[..valid_length])
                .map_err(|error| format!("description CLI returned non-UTF-8 output: {error}"))?;
            description.push_str(chunk);
            on_chunk(chunk)?;
            pending.drain(..valid_length);
        }
    }
    if !pending.is_empty() {
        return Err("description CLI ended with incomplete UTF-8 output".to_string());
    }
    Ok(description)
}

fn stream_claude_json(
    stdout: impl Read,
    on_chunk: impl Fn(&str) -> Result<(), String>,
) -> Result<String, String> {
    let mut description = String::new();
    let mut final_result = String::new();
    for line in BufReader::new(stdout).lines() {
        let line = line.map_err(|error| format!("failed to read Claude output: {error}"))?;
        let event: serde_json::Value = serde_json::from_str(&line)
            .map_err(|error| format!("Claude returned invalid stream JSON: {error}"))?;
        if let Some(text) = claude_text_delta(&event) {
            description.push_str(text);
            on_chunk(text)?;
        } else if event.get("type").and_then(serde_json::Value::as_str) == Some("result") {
            if event
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                return Err(event
                    .get("result")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Claude description failed")
                    .to_string());
            }
            final_result = event
                .get("result")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
    }
    if description.is_empty() && !final_result.is_empty() {
        on_chunk(&final_result)?;
        return Ok(final_result);
    }
    Ok(description)
}

fn claude_text_delta(event: &serde_json::Value) -> Option<&str> {
    let stream_event = event.get("event")?;
    (event.get("type")?.as_str()? == "stream_event"
        && stream_event.get("type")?.as_str()? == "content_block_delta"
        && stream_event.get("delta")?.get("type")?.as_str()? == "text_delta")
        .then(|| stream_event.get("delta")?.get("text")?.as_str())?
}

fn temporary_pi_session_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("coding-agent-va-pi-{}-{nonce}", std::process::id()))
}

fn build_description_prompt(
    request: &AgentSessionNodeDescriptionRequest,
    workspace_root: &Path,
) -> Result<String, String> {
    let mut nodes = vec![request.clicked_node.clone()];
    nodes.extend(
        request
            .related_nodes
            .iter()
            .take(MAX_RELATED_NODES)
            .cloned(),
    );
    let mut resolved_nodes = Vec::with_capacity(nodes.len());
    for node in nodes {
        let path = resolve_workspace_file(workspace_root, &node.path)?;
        resolved_nodes.push((node, path));
    }

    let mut prompt = String::from(
        "Describe the selected context-graph node using the supplied session context.\n\n",
    );
    prompt.push_str("## Selected node\n");
    append_node_metadata(&mut prompt, &request.clicked_node);
    prompt.push_str("\n## Related nodes\n");
    if request.related_nodes.is_empty() {
        prompt.push_str("None.\n");
    } else {
        for node in request.related_nodes.iter().take(MAX_RELATED_NODES) {
            append_node_metadata(&mut prompt, node);
        }
    }

    prompt.push_str("\n## Graph relationships\n");
    if request.relations.is_empty() {
        prompt.push_str("None.\n");
    } else {
        for relation in &request.relations {
            prompt.push_str(&format!(
                "- {} -> {} (import: {})\n",
                relation.source_path, relation.target_path, relation.import_specifier
            ));
        }
    }

    prompt.push_str("\n## Source excerpts\n");
    let mut remaining_source_chars = MAX_SOURCE_CHARS_TOTAL;
    for (node, path) in &resolved_nodes {
        if remaining_source_chars == 0 {
            break;
        }
        prompt.push_str(&format!("\n### {}\n", node.path));
        match fs::read_to_string(path) {
            Ok(source) => {
                let limit = MAX_SOURCE_CHARS_PER_FILE.min(remaining_source_chars);
                let (excerpt, truncated) = truncate_chars(&source, limit);
                prompt.push_str("```\n");
                prompt.push_str(excerpt);
                if truncated {
                    prompt.push_str("\n[truncated]");
                }
                prompt.push_str("\n```\n");
                remaining_source_chars =
                    remaining_source_chars.saturating_sub(excerpt.chars().count());
            }
            Err(_error) if !path.exists() => prompt.push_str("[file deleted or missing]\n"),
            Err(error) => prompt.push_str(&format!("[source unavailable: {error}]\n")),
        }
    }

    prompt.push_str("\n## Session edit history\n");
    prompt.push_str(&read_session_edit_history(
        request,
        workspace_root,
        &resolved_nodes,
    ));
    prompt.push_str("\n## Repository diff (HEAD to working tree)\n");
    prompt.push_str(&read_git_diff(workspace_root, &resolved_nodes));
    Ok(prompt)
}

fn append_node_metadata(prompt: &mut String, node: &DescriptionGraphNode) {
    let activities = if node.activities.is_empty() {
        "none".to_string()
    } else {
        node.activities.join(", ")
    };
    prompt.push_str(&format!(
        "- {} ({}, activities: {})\n",
        node.path, node.label, activities
    ));
}

fn read_git_diff(workspace_root: &Path, nodes: &[(DescriptionGraphNode, PathBuf)]) -> String {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(workspace_root)
        .arg("diff")
        .arg("--no-ext-diff")
        .arg("--unified=3")
        .arg("HEAD")
        .arg("--");
    for (_, path) in nodes {
        command.arg(path);
    }

    let Ok(output) = command.output() else {
        return "[git diff unavailable]\n".to_string();
    };
    if !output.status.success() {
        return format!(
            "[git diff unavailable: {}]\n",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    if output.stdout.is_empty() {
        return "[no tracked changes for supplied files; current source may include untracked files]\n"
            .to_string();
    }

    let diff = String::from_utf8_lossy(&output.stdout);
    let (excerpt, truncated) = truncate_chars(&diff, MAX_DIFF_CHARS);
    format!(
        "```diff\n{}{}\n```\n",
        excerpt,
        if truncated { "\n[truncated]" } else { "" }
    )
}

fn read_session_edit_history(
    request: &AgentSessionNodeDescriptionRequest,
    workspace_root: &Path,
    nodes: &[(DescriptionGraphNode, PathBuf)],
) -> String {
    let Ok(activity) = request
        .provider
        .protocol()
        .collect_file_activity(Path::new(&request.transcript_path), Some(&request.cwd))
    else {
        return "[session edit history unavailable]\n".to_string();
    };

    let selected_paths = nodes
        .iter()
        .map(|(_, path)| path)
        .collect::<std::collections::HashSet<_>>();
    let mut history = String::new();
    for (raw_path, fragments) in activity.edit_fragments {
        let Ok(path) = resolve_workspace_file(workspace_root, &raw_path) else {
            continue;
        };
        if !selected_paths.contains(&path) {
            continue;
        }

        history.push_str(&format!("\n### {raw_path}\n"));
        for fragment in fragments {
            if fragment.trim().is_empty() {
                history.push_str("[edit recorded without a textual fragment]\n");
            } else {
                history.push_str("```diff\n");
                history.push_str(&fragment);
                history.push_str("\n```\n");
            }
            if history.chars().count() >= MAX_SESSION_EDIT_CHARS {
                let (excerpt, _) = truncate_chars(&history, MAX_SESSION_EDIT_CHARS);
                return format!("{excerpt}\n[truncated]\n");
            }
        }
    }

    if history.is_empty() {
        "[no textual edit fragments recorded for supplied files]\n".to_string()
    } else {
        history
    }
}

fn canonical_workspace_root(cwd: &str) -> Result<PathBuf, String> {
    PathBuf::from(cwd)
        .canonicalize()
        .map_err(|error| format!("failed to resolve workspace {cwd}: {error}"))
}

fn resolve_workspace_file(workspace_root: &Path, raw_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw_path);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("node path escapes the workspace: {raw_path}"));
    }
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    let checked_path = candidate.canonicalize().unwrap_or(candidate);
    if !checked_path.starts_with(workspace_root) {
        return Err(format!("node path is outside the workspace: {raw_path}"));
    }
    Ok(checked_path)
}

fn truncate_chars(value: &str, max_chars: usize) -> (&str, bool) {
    match value.char_indices().nth(max_chars) {
        Some((index, _)) => (&value[..index], true),
        None => (value, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_session::types::{DescriptionGraphNode, DescriptionGraphRelation};

    #[test]
    fn provider_cli_invocations_resume_without_persisting() {
        let claude = cli_invocation(&sample_request(AgentSessionProvider::Claude));
        assert!(claude.args.contains(&OsString::from("--fork-session")));
        assert!(claude
            .args
            .contains(&OsString::from("--no-session-persistence")));
        assert!(claude.args.contains(&OsString::from("stream-json")));
        assert!(claude
            .args
            .contains(&OsString::from("--include-partial-messages")));
        assert!(claude
            .args
            .contains(&OsString::from(r#"{"alwaysThinkingEnabled":false}"#)));

        let pi = cli_invocation(&sample_request(AgentSessionProvider::Pi));
        assert!(pi.args.contains(&OsString::from("--fork")));
        assert!(pi.args.contains(&OsString::from("--session-dir")));
        assert!(!pi.args.contains(&OsString::from("--no-session")));
        assert!(pi.temporary_session_dir.is_some());
    }

    #[test]
    fn workspace_file_rejects_parent_traversal() {
        let root = std::env::current_dir().expect("current directory");
        let error = resolve_workspace_file(&root, "../secret").expect_err("reject traversal");
        assert!(error.contains("escapes the workspace"));
    }

    #[test]
    fn char_truncation_preserves_utf8_boundaries() {
        assert_eq!(truncate_chars("가나다", 2), ("가나", true));
        assert_eq!(truncate_chars("가나다", 3), ("가나다", false));
    }

    #[test]
    fn plain_text_stream_preserves_split_utf8_characters() {
        struct OneByteReader(std::io::Cursor<Vec<u8>>);
        impl Read for OneByteReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let read_length = buffer.len().min(1);
                self.0.read(&mut buffer[..read_length])
            }
        }

        let streamed = std::cell::RefCell::new(String::new());
        let output = stream_plain_text(
            OneByteReader(std::io::Cursor::new("한글".into())),
            |text| {
                streamed.borrow_mut().push_str(text);
                Ok(())
            },
        )
        .expect("stream plain text");

        assert_eq!(output, "한글");
        assert_eq!(*streamed.borrow(), "한글");
    }

    #[test]
    fn claude_stream_extracts_partial_text_deltas() {
        let input = concat!(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello "}}}"#,
            "\n",
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"world"}}}"#,
            "\n",
            r#"{"type":"result","is_error":false,"result":"Hello world"}"#,
            "\n"
        );
        let streamed = std::cell::RefCell::new(String::new());
        let output = stream_claude_json(input.as_bytes(), |text| {
            streamed.borrow_mut().push_str(text);
            Ok(())
        })
        .expect("stream Claude JSON");

        assert_eq!(output, "Hello world");
        assert_eq!(*streamed.borrow(), "Hello world");
    }

    #[test]
    fn description_cache_reuses_matching_context() {
        let cache = NodeDescriptionCacheState::default();
        let response = AgentSessionNodeDescriptionResponse {
            description: "cached".to_string(),
            provider_label: "Codex".to_string(),
        };

        cache.insert(7, response).expect("insert cache entry");
        let cached = cache.get(7).expect("read cache entry").expect("cache hit");

        assert_eq!(cached.description, "cached");
        assert!(cache.get(8).expect("read cache miss").is_none());
    }

    #[test]
    fn description_cache_key_changes_with_injected_context() {
        let request = sample_request(AgentSessionProvider::Codex);

        assert_ne!(
            description_cache_key(&request, "source before"),
            description_cache_key(&request, "source after")
        );
    }

    #[test]
    fn related_file_change_invalidates_description_cache_key() {
        let workspace = std::env::temp_dir().join(format!(
            "coding-agent-va-description-cache-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(workspace.join("src")).expect("create test workspace");
        fs::write(workspace.join("src/main.rs"), "mod related;\n").expect("write selected file");
        fs::write(
            workspace.join("src/related.rs"),
            "pub fn value() -> u8 { 1 }\n",
        )
        .expect("write related file");
        fs::write(workspace.join("session.jsonl"), "").expect("write transcript");

        let mut request = sample_request(AgentSessionProvider::Codex);
        request.cwd = workspace.display().to_string();
        request.transcript_path = workspace.join("session.jsonl").display().to_string();
        request.related_nodes = vec![DescriptionGraphNode {
            label: "related.rs".to_string(),
            path: "src/related.rs".to_string(),
            activities: vec!["impacted".to_string()],
        }];
        let root = canonical_workspace_root(&request.cwd).expect("resolve workspace");
        let before = build_description_prompt(&request, &root).expect("build initial prompt");

        fs::write(
            workspace.join("src/related.rs"),
            "pub fn value() -> u8 { 2 }\n",
        )
        .expect("update related file");
        let after = build_description_prompt(&request, &root).expect("build updated prompt");

        assert_ne!(
            description_cache_key(&request, &before),
            description_cache_key(&request, &after)
        );
        fs::remove_dir_all(workspace).expect("remove test workspace");
    }

    fn sample_request(provider: AgentSessionProvider) -> AgentSessionNodeDescriptionRequest {
        AgentSessionNodeDescriptionRequest {
            provider,
            provider_session_id: "session-id".to_string(),
            transcript_path: "/tmp/session.jsonl".to_string(),
            runtime_home: "/tmp/runtime".to_string(),
            model: "test-model".to_string(),
            reasoning: DescriptionReasoning::None,
            cwd: "/tmp".to_string(),
            clicked_node: DescriptionGraphNode {
                label: "main.rs".to_string(),
                path: "src/main.rs".to_string(),
                activities: vec!["edited".to_string()],
            },
            related_nodes: Vec::new(),
            relations: vec![DescriptionGraphRelation {
                source_path: "src/main.rs".to_string(),
                target_path: "src/lib.rs".to_string(),
                import_specifier: "crate::lib".to_string(),
            }],
        }
    }
}
