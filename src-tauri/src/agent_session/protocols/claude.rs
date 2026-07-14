use std::path::{Path, PathBuf};

use rayon::prelude::*;

use super::{build_session_summary, AgentSessionCandidate, AgentSessionProtocol};
use crate::agent_session::activity::{
    read_tool_call_file_activity, ActivityAccumulator, ToolSchema,
};
use crate::agent_session::file_system::{
    directory_title, file_stem_string, file_updated_at_ms, is_jsonl_file_name, list_jsonl_files,
};
use crate::agent_session::titles::{
    read_claude_ai_title, read_claude_first_user_prompt_title, read_claude_session_metadata,
};
use crate::agent_session::types::{AgentSessionProvider, AgentSessionSummary};

pub(crate) struct ClaudeSessionProtocol;

impl AgentSessionProtocol for ClaudeSessionProtocol {
    fn provider(&self) -> AgentSessionProvider {
        AgentSessionProvider::Claude
    }

    fn default_runtime_home(&self) -> Option<PathBuf> {
        crate::agent_session::file_system::home_dir().map(|home| home.join(".claude"))
    }

    fn list_session_candidates(&self, runtime_home: &Path) -> Vec<AgentSessionCandidate> {
        list_jsonl_files(&runtime_home.join("projects"))
            .into_iter()
            .filter(|path| {
                !path
                    .components()
                    .any(|component| component.as_os_str() == "subagents")
            })
            .map(|transcript_path| AgentSessionCandidate {
                updated_at_ms: file_updated_at_ms(&transcript_path),
                transcript_path,
            })
            .collect()
    }

    fn hydrate_sessions(
        &self,
        runtime_home: &Path,
        candidates: &[AgentSessionCandidate],
    ) -> Vec<AgentSessionSummary> {
        candidates
            .par_iter()
            .filter_map(|candidate| {
                let path = &candidate.transcript_path;
                let metadata = read_claude_session_metadata(path)?;
                let provider_session_id = metadata.session_id.or_else(|| file_stem_string(path))?;
                let title = read_claude_ai_title(path)
                    .or_else(|| read_claude_first_user_prompt_title(path))
                    .or_else(|| metadata.cwd.as_deref().and_then(directory_title))
                    .unwrap_or_else(|| provider_session_id.clone());

                Some(build_session_summary(
                    self.provider(),
                    provider_session_id,
                    title,
                    path.clone(),
                    metadata.cwd,
                    runtime_home,
                    candidate.updated_at_ms,
                ))
            })
            .collect()
    }

    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)> {
        vec![
            (
                runtime_home.join("history.jsonl"),
                false,
                "watch Claude prompt history updates".to_string(),
            ),
            (
                runtime_home.join("projects"),
                true,
                "watch Claude project transcripts".to_string(),
            ),
        ]
    }

    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool {
        let Ok(relative_path) = path.strip_prefix(runtime_home) else {
            return false;
        };

        if relative_path == Path::new("history.jsonl") {
            return true;
        }

        relative_path
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
            == Some("projects")
            && is_jsonl_file_name(path, None)
    }

    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String> {
        read_tool_call_file_activity(transcript_path, cwd, ToolSchema::Claude)
    }
}
