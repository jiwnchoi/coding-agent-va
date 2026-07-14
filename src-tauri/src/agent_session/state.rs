use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::async_runtime::JoinHandle;

use super::protocols::AgentSessionCandidate;
use super::types::{AgentSessionFileActivity, AgentSessionProvider, AgentSessionSummary};

#[derive(Clone, Default)]
pub struct AgentSessionWatchState {
    pub(crate) watches: Arc<Mutex<HashMap<String, SessionWatchHandle>>>,
    pub(crate) session_candidates:
        Arc<Mutex<HashMap<SessionListCacheKey, Vec<AgentSessionCandidate>>>>,
    pub(crate) loaded_sessions: Arc<Mutex<HashMap<SessionListCacheKey, Vec<AgentSessionSummary>>>>,
    pub(crate) file_activities: Arc<Mutex<HashMap<FileActivityCacheKey, FileActivityCacheEntry>>>,
}

pub(crate) struct SessionWatchHandle {
    pub(crate) task: JoinHandle<()>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct SessionListCacheKey {
    pub(crate) provider: AgentSessionProvider,
    pub(crate) runtime_home: String,
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct FileActivityCacheKey {
    pub(crate) provider: AgentSessionProvider,
    pub(crate) transcript_path: String,
    pub(crate) cwd: Option<String>,
    pub(crate) hide_committed_files: bool,
}

#[derive(Clone)]
pub(crate) struct FileActivityCacheEntry {
    pub(crate) transcript_updated_at_ms: u64,
    pub(crate) git_index_updated_at_ms: Option<u64>,
    pub(crate) activity: AgentSessionFileActivity,
}

impl Drop for SessionWatchHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}
