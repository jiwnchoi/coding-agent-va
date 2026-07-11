pub(crate) mod codex;
mod shell;
mod tool_calls;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub(crate) use tool_calls::{read_tool_call_file_activity, ToolSchema};

use crate::indexer::workspace_dependencies::{
    find_impacted_file_relations, ImpactedFileRelation as WorkspaceImpactedFileRelation,
};

use super::git::read_git_worktree_status;
use super::paths::normalize_written_activity_path;
use super::types::AgentSessionImpactedFileRelation;

#[derive(Default)]
pub(crate) struct ActivityAccumulator {
    pub(crate) read_files: HashMap<String, u64>,
    pub(crate) edited_files: HashMap<String, u64>,
    pub(crate) deleted_files: HashMap<String, u64>,
}

pub(crate) fn sort_file_activity(activity: HashMap<String, u64>) -> Vec<String> {
    let mut entries = activity.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    entries.into_iter().map(|(path, _)| path).collect()
}

pub(crate) fn resolve_impacted_file_relations(
    cwd: Option<&str>,
    edited_files: &HashMap<String, u64>,
    deleted_files: &HashMap<String, u64>,
) -> Result<Vec<AgentSessionImpactedFileRelation>, String> {
    let Some(workspace_root) = cwd.map(PathBuf::from) else {
        return Ok(Vec::new());
    };

    let changed_files = edited_files
        .keys()
        .chain(deleted_files.keys())
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    find_impacted_file_relations(&workspace_root, &changed_files).map(|relations| {
        relations
            .into_iter()
            .map(agent_impacted_file_relation_from_workspace)
            .collect()
    })
}

pub(crate) fn remove_edited_files_from_read_files(
    cwd: Option<&str>,
    read_files: &mut HashMap<String, u64>,
    edited_files: &HashMap<String, u64>,
) {
    let edited_paths = edited_files
        .keys()
        .filter_map(|path| normalize_written_activity_path(path, cwd))
        .collect::<HashSet<_>>();

    read_files.retain(|path, _| {
        normalize_written_activity_path(path, cwd)
            .map(|normalized_path| !edited_paths.contains(&normalized_path))
            .unwrap_or(true)
    });
}

pub(crate) fn filter_written_files_by_git_status(
    cwd: Option<&str>,
    edited_files: &mut HashMap<String, u64>,
    deleted_files: &mut HashMap<String, u64>,
) {
    let candidate_paths = edited_files
        .keys()
        .chain(deleted_files.keys())
        .filter_map(|path| normalize_written_activity_path(path, cwd))
        .collect::<HashSet<_>>();
    let Some(git_status) = read_git_worktree_status(&candidate_paths) else {
        return;
    };

    edited_files.retain(|path, _| {
        normalize_written_activity_path(path, cwd)
            .is_some_and(|normalized_path| git_status.edited_files.contains(&normalized_path))
    });
    deleted_files.retain(|path, _| {
        normalize_written_activity_path(path, cwd)
            .is_some_and(|normalized_path| git_status.deleted_files.contains(&normalized_path))
    });
}

fn agent_impacted_file_relation_from_workspace(
    relation: WorkspaceImpactedFileRelation,
) -> AgentSessionImpactedFileRelation {
    AgentSessionImpactedFileRelation {
        changed_file: relation.changed_file,
        impacted_file: relation.impacted_file,
        import_specifier: relation.import_specifier,
    }
}
