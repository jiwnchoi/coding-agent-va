mod claude;
mod codex;
mod pi;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) use claude::ClaudeSessionProtocol;
pub(crate) use codex::CodexSessionProtocol;
pub(crate) use pi::PiSessionProtocol;

use super::activity::{
    filter_written_files_by_git_status, remove_edited_files_from_read_files,
    resolve_impacted_file_relations, sort_file_activity, ActivityAccumulator,
};
use super::titles::normalize_title;
use super::types::{
    AgentRuntimeSource, AgentSessionFileActivity, AgentSessionProvider, AgentSessionSummary,
};

pub(crate) trait AgentSessionProtocol: Send + Sync {
    fn provider(&self) -> AgentSessionProvider;
    fn default_runtime_home(&self) -> Option<PathBuf>;
    fn list_sessions(&self, runtime_home: &Path) -> Vec<AgentSessionSummary>;
    fn watch_roots(&self, runtime_home: &Path) -> Vec<(PathBuf, bool, String)>;
    fn is_relevant_session_path(&self, path: &Path, runtime_home: &Path) -> bool;
    fn collect_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
    ) -> Result<ActivityAccumulator, String>;

    fn read_file_activity(
        &self,
        transcript_path: &Path,
        cwd: Option<&str>,
        hide_committed_files: bool,
    ) -> Result<AgentSessionFileActivity, String> {
        let mut activity = self.collect_file_activity(transcript_path, cwd)?;

        if hide_committed_files {
            filter_written_files_by_git_status(
                cwd,
                &mut activity.edited_files,
                &mut activity.deleted_files,
            );
            activity
                .edit_fragments
                .retain(|path, _| activity.edited_files.contains_key(path));
        }
        remove_edited_files_from_read_files(cwd, &mut activity.read_files, &activity.edited_files);
        let impacted_relations = resolve_impacted_file_relations(
            cwd,
            &activity.edited_files,
            &activity.deleted_files,
            &activity.edit_fragments,
        )?;
        let impacted_files = impacted_relations
            .iter()
            .map(|relation| relation.impacted_file.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        Ok(AgentSessionFileActivity {
            read_files: sort_file_activity(activity.read_files),
            edited_files: sort_file_activity(activity.edited_files),
            impacted_files,
            deleted_files: sort_file_activity(activity.deleted_files),
            impacted_relations,
        })
    }
}

impl AgentSessionProvider {
    pub(crate) fn protocol(self) -> Box<dyn AgentSessionProtocol> {
        match self {
            Self::Codex => Box::new(CodexSessionProtocol),
            Self::Claude => Box::new(ClaudeSessionProtocol),
            Self::Pi => Box::new(PiSessionProtocol),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::Pi => "Pi Agent",
        }
    }

    pub(crate) fn all() -> [Self; 3] {
        [Self::Codex, Self::Claude, Self::Pi]
    }
}

pub(crate) fn default_runtime_sources(
    runtime_home_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<AgentRuntimeSource> {
    AgentSessionProvider::all()
        .into_iter()
        .filter_map(|provider| {
            let protocol = provider.protocol();
            let default_runtime_home = protocol.default_runtime_home()?;
            let runtime_home = runtime_home_overrides
                .and_then(|overrides| overrides.get(provider_key(provider)))
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or(default_runtime_home);
            Some(AgentRuntimeSource {
                provider,
                label: provider.label().to_string(),
                runtime_home: runtime_home.display().to_string(),
                available: runtime_home.is_dir(),
            })
        })
        .collect()
}

pub(crate) fn build_session_summary(
    provider: AgentSessionProvider,
    provider_session_id: String,
    title: String,
    transcript_path: PathBuf,
    cwd: Option<String>,
    runtime_home: &Path,
    updated_at_ms: u64,
) -> AgentSessionSummary {
    AgentSessionSummary {
        id: format!("{}:{}", provider_key(provider), provider_session_id),
        provider,
        provider_session_id,
        provider_label: provider.label().to_string(),
        title: normalize_title(title),
        transcript_path: transcript_path.display().to_string(),
        cwd,
        runtime_home: runtime_home.display().to_string(),
        updated_at_ms,
    }
}

pub(crate) fn provider_key(provider: AgentSessionProvider) -> &'static str {
    match provider {
        AgentSessionProvider::Codex => "codex",
        AgentSessionProvider::Claude => "claude",
        AgentSessionProvider::Pi => "pi",
    }
}
