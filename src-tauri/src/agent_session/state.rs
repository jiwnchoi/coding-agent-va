use std::collections::HashMap;
use std::sync::Mutex;

use tauri::async_runtime::JoinHandle;

use super::types::{AgentSessionFileActivity, AgentSessionProvider, AgentSessionSummary};

#[derive(Default)]
pub struct AgentSessionWatchState {
    pub(crate) watches: Mutex<HashMap<String, SessionWatchHandle>>,
    pub(crate) session_lists: Mutex<HashMap<SessionListCacheKey, Vec<AgentSessionSummary>>>,
    pub(crate) file_activities: Mutex<HashMap<FileActivityCacheKey, FileActivityCacheEntry>>,
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
