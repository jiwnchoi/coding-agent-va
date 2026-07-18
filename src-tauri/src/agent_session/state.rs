use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::async_runtime::JoinHandle;

use super::protocols::AgentSessionCandidate;
use super::types::{AgentSessionProvider, AgentSessionSummary};

#[derive(Clone, Default)]
pub struct AgentSessionWatchState {
    pub(crate) watches: Arc<Mutex<HashMap<String, SessionWatchHandle>>>,
    pub(crate) session_candidates:
        Arc<Mutex<HashMap<SessionListCacheKey, Vec<AgentSessionCandidate>>>>,
    pub(crate) loaded_sessions: Arc<Mutex<HashMap<SessionListCacheKey, Vec<AgentSessionSummary>>>>,
}

pub(crate) struct SessionWatchHandle {
    pub(crate) task: JoinHandle<()>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct SessionListCacheKey {
    pub(crate) provider: AgentSessionProvider,
    pub(crate) runtime_home: String,
}

impl Drop for SessionWatchHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}
