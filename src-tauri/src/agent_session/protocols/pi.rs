use std::path::{Path, PathBuf};

use rayon::prelude::*;

use super::{build_session_summary, AgentSessionCandidate, AgentSessionProtocol};
use crate::agent_session::activity::{
    read_tool_call_file_activity, ActivityAccumulator, ToolSchema,
};
use crate::agent_session::file_system::{
    directory_title, file_stem_string, file_updated_at_ms, is_jsonl_file_name, list_jsonl_files,
};
use crate::agent_session::json::{json_str, read_first_json_line};
use crate::agent_session::titles::{read_pi_first_user_prompt_title, read_pi_session_name};
use crate::agent_session::types::{AgentSessionProvider, AgentSessionSummary};

pub(crate) struct PiSessionProtocol;

impl AgentSessionProtocol for PiSessionProtocol {
    fn provider(&self) -> AgentSessionProvider {
        AgentSessionProvider::Pi
    }

    fn default_runtime_home(&self) -> Option<PathBuf> {
        crate::agent_session::file_system::home_dir().map(|home| home.join(".pi").join("agent"))
    }

    fn list_session_candidates(&self, runtime_home: &Path) -> Vec<AgentSessionCandidate> {
        list_jsonl_files(&runtime_home.join("sessions"))
            .into_iter()
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
                let header = read_first_json_line(path)?;
                if json_str(&header, &["type"]) != Some("session") {
                    return None;
                }

                let provider_session_id = json_str(&header, &["id"])
                    .map(str::to_string)
                    .or_else(|| file_stem_string(path))?;
                let cwd = json_str(&header, &["cwd"]).map(str::to_string);
                let title = read_pi_session_name(path)
                    .or_else(|| read_pi_first_user_prompt_title(path))
                    .or_else(|| cwd.as_deref().and_then(directory_title))
                    .unwrap_or_else(|| provider_session_id.clone());

                Some(build_session_summary(
                    self.provider(),
                    provider_session_id,
                    title,
                    path.clone(),
                    cwd,
                    runtime_home,
                    candidate.updated_at_ms,
                ))
            })
            .collect()
    }

    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)> {
        vec![(
            runtime_home.join("sessions"),
            true,
            "watch Pi session transcripts".to_string(),
        )]
    }

    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool {
        path.strip_prefix(runtime_home)
            .ok()
            .and_then(|relative_path| relative_path.components().next())
            .and_then(|component| component.as_os_str().to_str())
            == Some("sessions")
            && is_jsonl_file_name(path, None)
    }

    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String> {
        read_tool_call_file_activity(transcript_path, cwd, ToolSchema::Pi)
    }
}
