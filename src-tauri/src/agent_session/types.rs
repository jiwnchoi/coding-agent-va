use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app_config::DescriptionReasoning;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum AgentSessionProvider {
    Codex,
    Claude,
    Pi,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeSource {
    pub provider: AgentSessionProvider,
    pub label: String,
    pub runtime_home: String,
    pub available: bool,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionSummary {
    pub id: String,
    pub provider: AgentSessionProvider,
    pub provider_session_id: String,
    pub provider_label: String,
    pub title: String,
    pub transcript_path: String,
    pub cwd: Option<String>,
    pub runtime_home: String,
    pub updated_at_ms: u64,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionList {
    pub sources: Vec<AgentRuntimeSource>,
    pub sessions: Vec<AgentSessionSummary>,
    pub has_more: bool,
    pub next_offset: usize,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchTarget {
    pub path: String,
    pub recursive: bool,
    pub exists: bool,
    pub reason: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchPlan {
    pub watch_id: String,
    pub provider: AgentSessionProvider,
    pub runtime_home: String,
    pub watch_targets: Vec<SessionWatchTarget>,
    pub git_index_paths: Vec<String>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchRegistration {
    pub watch_id: String,
    pub provider: AgentSessionProvider,
    pub runtime_home: String,
    pub watch_targets: Vec<SessionWatchTarget>,
    pub git_index_paths: Vec<String>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionWatchEventPayload {
    pub watch_id: String,
    pub provider: AgentSessionProvider,
    pub runtime_home: String,
    pub changed_paths: Vec<String>,
    pub event_tags: Vec<String>,
    pub timestamp_ms: u64,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionFileActivity {
    pub read_files: Vec<String>,
    pub edited_files: Vec<String>,
    pub impacted_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub impacted_relations: Vec<AgentSessionImpactedFileRelation>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionImpactedFileRelation {
    pub changed_file: String,
    pub impacted_file: String,
    pub import_specifier: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionFileDiff {
    pub file_path: String,
    pub display_path: String,
    pub original_content: String,
    pub modified_content: String,
    pub diff_base_label: String,
    pub diff_target_label: String,
    pub file_missing: bool,
    pub is_tracked: bool,
}

#[derive(Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DescriptionGraphNode {
    pub label: String,
    pub path: String,
    pub activities: Vec<String>,
}

#[derive(Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DescriptionGraphRelation {
    pub source_path: String,
    pub target_path: String,
    pub import_specifier: String,
}

#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionNodeDescriptionRequest {
    pub provider: AgentSessionProvider,
    pub provider_session_id: String,
    pub transcript_path: String,
    pub runtime_home: String,
    pub model: String,
    pub reasoning: DescriptionReasoning,
    pub cwd: String,
    pub clicked_node: DescriptionGraphNode,
    pub related_nodes: Vec<DescriptionGraphNode>,
    pub relations: Vec<DescriptionGraphRelation>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionNodeDescriptionResponse {
    pub description: String,
    pub provider_label: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentSessionNodeDescriptionStreamEvent {
    Started {
        provider_label: String,
        cached: bool,
    },
    Chunk {
        text: String,
    },
}
