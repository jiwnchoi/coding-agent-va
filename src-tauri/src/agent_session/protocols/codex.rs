use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use super::{build_session_summary, AgentSessionProtocol};
use crate::agent_session::activity::codex::{
    collect_codex_read_files, collect_codex_written_files, CodexRolloutEnvelope,
};
use crate::agent_session::activity::ActivityAccumulator;
use crate::agent_session::file_system::{
    directory_title, file_updated_at_ms, is_jsonl_file_name, list_jsonl_files,
};
use crate::agent_session::time::entry_timestamp_ms;
use crate::agent_session::titles::{
    extract_codex_session_id, read_codex_first_user_prompt_title, read_codex_session_meta_cwd,
    read_codex_session_titles,
};
use crate::agent_session::types::{AgentSessionProvider, AgentSessionSummary};

pub(crate) struct CodexSessionProtocol;

impl AgentSessionProtocol for CodexSessionProtocol {
    fn provider(&self) -> AgentSessionProvider {
        AgentSessionProvider::Codex
    }

    fn default_runtime_home(&self) -> Option<PathBuf> {
        crate::agent_session::file_system::home_dir().map(|home| home.join(".codex"))
    }

    fn list_sessions(&self, runtime_home: &Path) -> Vec<AgentSessionSummary> {
        let session_titles = read_codex_session_titles(runtime_home);
        let sessions_dir = runtime_home.join("sessions");

        list_jsonl_files(&sessions_dir)
            .into_par_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
            })
            .filter_map(|path| {
                let file_name = path.file_name()?.to_str()?;
                let provider_session_id = extract_codex_session_id(file_name)?;
                let updated_at_ms = file_updated_at_ms(&path);
                let cwd = read_codex_session_meta_cwd(&path);
                let title = read_codex_first_user_prompt_title(&path)
                    .or_else(|| {
                        session_titles
                            .get(&provider_session_id)
                            .cloned()
                            .map(crate::agent_session::titles::normalize_title_whitespace)
                    })
                    .or_else(|| cwd.as_deref().and_then(directory_title))
                    .unwrap_or_else(|| provider_session_id.clone());

                Some(build_session_summary(
                    self.provider(),
                    provider_session_id,
                    title,
                    path,
                    cwd,
                    runtime_home,
                    updated_at_ms,
                ))
            })
            .collect()
    }

    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)> {
        vec![
            (
                runtime_home.join("state_5.sqlite"),
                false,
                "watch SQLite index updates".to_string(),
            ),
            (
                runtime_home.join("state_5.sqlite-wal"),
                false,
                "watch SQLite WAL updates".to_string(),
            ),
            (
                runtime_home.join("history.jsonl"),
                false,
                "watch prompt history updates".to_string(),
            ),
            (
                runtime_home.join("sessions"),
                true,
                "watch rollout session trees".to_string(),
            ),
        ]
    }

    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool {
        let Ok(relative_path) = path.strip_prefix(runtime_home) else {
            return false;
        };

        if relative_path == Path::new("state_5.sqlite")
            || relative_path == Path::new("state_5.sqlite-wal")
            || relative_path == Path::new("history.jsonl")
        {
            return true;
        }

        relative_path
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
            == Some("sessions")
            && is_jsonl_file_name(path, Some("rollout-"))
    }

    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String> {
        let file = File::open(transcript_path).map_err(|error| {
            format!(
                "failed to open Codex rollout {}: {error}",
                transcript_path.display()
            )
        })?;
        let workspace_root = cwd.map(PathBuf::from);
        let mut activity = ActivityAccumulator::default();

        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let timestamp_ms = entry_timestamp_ms(&entry);
            let Ok(envelope) = serde_json::from_value::<CodexRolloutEnvelope>(entry) else {
                continue;
            };

            match envelope.entry_type.as_str() {
                "response_item" => {
                    collect_codex_read_files(
                        &envelope.payload,
                        workspace_root.as_deref(),
                        timestamp_ms,
                        &mut activity.read_files,
                    );
                    collect_codex_written_files(
                        &envelope.payload,
                        workspace_root.as_deref(),
                        timestamp_ms,
                        &mut activity,
                    );
                }
                "event_msg" => collect_codex_written_files(
                    &envelope.payload,
                    workspace_root.as_deref(),
                    timestamp_ms,
                    &mut activity,
                ),
                _ => {}
            }
        }

        Ok(activity)
    }
}
