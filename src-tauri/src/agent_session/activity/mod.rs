pub(crate) mod codex;
mod shell;
mod tool_calls;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(crate) use tool_calls::{
    collect_tool_call_entry_activity, read_tool_call_file_activity, ToolSchema,
};

use crate::indexer::workspace_dependencies::{
    find_session_impacted_file_relations, ImpactedFileRelation as WorkspaceImpactedFileRelation,
    SessionFileEdit,
};

use super::paths::normalize_written_activity_path;
use super::types::AgentSessionFileActivity;
use super::types::AgentSessionImpactedFileRelation;

#[derive(Clone, Default)]
pub(crate) struct ActivityAccumulator {
    pub(crate) read_files: HashMap<String, u64>,
    pub(crate) edited_files: HashMap<String, u64>,
    pub(crate) deleted_files: HashMap<String, u64>,
    pub(crate) edit_fragments: HashMap<String, Vec<String>>,
}

impl ActivityAccumulator {
    pub(crate) fn record_edit_fragment(
        &mut self,
        path: &str,
        workspace_root: Option<&std::path::Path>,
        fragment: Option<&str>,
    ) {
        let path = workspace_root
            .and_then(|root| normalize_written_activity_path(path, root.to_str()))
            .unwrap_or_else(|| path.to_string());
        let fragments = self.edit_fragments.entry(path).or_default();
        if let Some(fragment) = fragment.filter(|value| !value.trim().is_empty()) {
            fragments.push(fragment.to_string());
        } else if !fragments.iter().any(String::is_empty) {
            fragments.push(String::new());
        }
    }

    pub(crate) fn retain_workspace_paths(&mut self, cwd: Option<&str>) {
        let Some(workspace_root) = canonical_workspace_root(cwd) else {
            return;
        };

        self.read_files
            .retain(|path, _| path_is_in_workspace(path, &workspace_root));
        self.edited_files
            .retain(|path, _| path_is_in_workspace(path, &workspace_root));
        self.deleted_files
            .retain(|path, _| path_is_in_workspace(path, &workspace_root));
        self.edit_fragments
            .retain(|path, _| path_is_in_workspace(path, &workspace_root));
    }
}

pub(crate) fn finish_file_activity(
    cwd: Option<&str>,
    mut activity: ActivityAccumulator,
) -> AgentSessionFileActivity {
    activity.retain_workspace_paths(cwd);
    remove_edited_files_from_read_files(cwd, &mut activity.read_files, &activity.edited_files);
    AgentSessionFileActivity {
        read_files: sort_file_activity(activity.read_files),
        edited_files: sort_file_activity(activity.edited_files),
        impacted_files: Vec::new(),
        deleted_files: sort_file_activity(activity.deleted_files),
        impacted_relations: Vec::new(),
    }
}

fn canonical_workspace_root(cwd: Option<&str>) -> Option<PathBuf> {
    let workspace_root = PathBuf::from(cwd?);
    Some(workspace_root.canonicalize().unwrap_or(workspace_root))
}

fn path_is_in_workspace(path: &str, workspace_root: &Path) -> bool {
    normalize_written_activity_path(path, workspace_root.to_str())
        .is_some_and(|path| Path::new(&path).starts_with(workspace_root))
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
    edit_fragments: &HashMap<String, Vec<String>>,
) -> Result<Vec<AgentSessionImpactedFileRelation>, String> {
    let Some(workspace_root) = cwd.map(PathBuf::from) else {
        return Ok(Vec::new());
    };

    let edits = edited_files
        .keys()
        .filter(|path| path_is_in_workspace(path, &workspace_root))
        .map(|path| SessionFileEdit {
            path: PathBuf::from(path),
            fragments: edit_fragments.get(path).cloned().unwrap_or_default(),
        })
        .chain(
            deleted_files
                .keys()
                .filter(|path| path_is_in_workspace(path, &workspace_root))
                .map(|path| SessionFileEdit {
                    path: PathBuf::from(path),
                    fragments: Vec::new(),
                }),
        )
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return Ok(Vec::new());
    }

    find_session_impacted_file_relations(&workspace_root, &edits).map(|relations| {
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

fn agent_impacted_file_relation_from_workspace(
    relation: WorkspaceImpactedFileRelation,
) -> AgentSessionImpactedFileRelation {
    AgentSessionImpactedFileRelation {
        changed_file: relation.changed_file,
        impacted_file: relation.impacted_file,
        import_specifier: relation.import_specifier,
    }
}

#[cfg(test)]
mod tests {
    use super::{finish_file_activity, ActivityAccumulator};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn file_activity_only_contains_the_session_worktree() {
        let test_root = std::env::temp_dir().join(format!(
            "coding-agent-va-worktree-filter-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let session_worktree = test_root.join("current");
        let other_worktree = test_root.join("other");
        fs::create_dir_all(session_worktree.join("src")).expect("create session worktree");
        fs::create_dir_all(other_worktree.join("src")).expect("create other worktree");
        let session_file = session_worktree.join("src/current.ts");
        let other_file = other_worktree.join("src/other.ts");
        fs::write(&session_file, "current").expect("write session file");
        fs::write(&other_file, "other").expect("write other file");
        let mut activity = ActivityAccumulator::default();
        activity
            .read_files
            .insert(session_file.display().to_string(), 1);
        activity
            .read_files
            .insert(other_file.display().to_string(), 2);
        activity
            .edited_files
            .insert(other_file.display().to_string(), 2);
        activity.edit_fragments.insert(
            other_file.display().to_string(),
            vec!["changed".to_string()],
        );

        let file_activity = finish_file_activity(session_worktree.to_str(), activity);

        assert_eq!(
            file_activity.read_files,
            vec![session_file.display().to_string()]
        );
        assert!(file_activity.edited_files.is_empty());
        fs::remove_dir_all(test_root).expect("remove test worktrees");
    }
}
