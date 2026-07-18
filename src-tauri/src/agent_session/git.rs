use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rayon::prelude::*;

use super::paths::normalize_absolute_activity_path;
use super::types::{AgentSessionFileDiff, AgentSessionSummary};

pub(crate) fn read_agent_session_file_diff(
    file_path: &Path,
    cwd: Option<&str>,
) -> Result<AgentSessionFileDiff, String> {
    let absolute_file_path = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        let workspace_root = cwd
            .map(PathBuf::from)
            .ok_or_else(|| format!("relative file path requires cwd: {}", file_path.display()))?;
        workspace_root.join(file_path)
    };
    let display_path = cwd
        .and_then(|workspace_root| {
            absolute_file_path
                .strip_prefix(workspace_root)
                .ok()
                .map(|relative_path| relative_path.display().to_string())
        })
        .unwrap_or_else(|| absolute_file_path.display().to_string());
    let modified_content = fs::read_to_string(&absolute_file_path).unwrap_or_default();
    let file_missing = !absolute_file_path.is_file();
    let git_snapshot = read_git_head_snapshot(&absolute_file_path, cwd);
    let (original_content, is_tracked) = match git_snapshot {
        Some(content) => (content, true),
        None if file_missing => (String::new(), false),
        None => (modified_content.clone(), false),
    };

    Ok(AgentSessionFileDiff {
        file_path: absolute_file_path.display().to_string(),
        display_path,
        original_content,
        modified_content,
        diff_base_label: "HEAD".to_string(),
        diff_target_label: if file_missing {
            "Deleted".to_string()
        } else if is_tracked {
            "Working tree".to_string()
        } else {
            "Workspace".to_string()
        },
        file_missing,
        is_tracked,
    })
}

pub(crate) fn collect_git_index_paths_from_sessions(
    sessions: &[AgentSessionSummary],
) -> Vec<PathBuf> {
    let mut cwd_paths = BTreeSet::<PathBuf>::new();
    for session in sessions {
        let Some(cwd) = &session.cwd else {
            continue;
        };

        cwd_paths.insert(PathBuf::from(cwd));
    }

    cwd_paths
        .into_par_iter()
        .filter_map(|cwd| resolve_git_repo_root_from_path(&cwd))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|repo_root| {
            resolve_git_index_path(&repo_root).unwrap_or_else(|| repo_root.join(".git/index"))
        })
        .collect()
}

fn read_git_head_snapshot(file_path: &Path, cwd: Option<&str>) -> Option<String> {
    let repo_root = resolve_git_repo_root(file_path, cwd)?;
    let relative_path = file_path.strip_prefix(&repo_root).ok()?;
    let git_object_path = relative_path.to_string_lossy().replace('\\', "/");
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .arg("show")
        .arg(format!("HEAD:{git_object_path}"))
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn resolve_git_repo_root(file_path: &Path, cwd: Option<&str>) -> Option<PathBuf> {
    let search_root = if file_path.is_file() {
        file_path.parent().map(Path::to_path_buf)
    } else {
        Some(file_path.to_path_buf())
    }
    .or_else(|| cwd.map(PathBuf::from))?;
    resolve_git_repo_root_from_path(&search_root)
}

fn resolve_git_repo_root_from_path(search_root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(search_root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let repo_root = String::from_utf8(output.stdout).ok()?;
    let trimmed = repo_root.trim();
    if trimmed.is_empty() {
        return None;
    }

    let repo_root = PathBuf::from(trimmed);
    Some(repo_root.canonicalize().unwrap_or(repo_root))
}

fn resolve_git_index_path(repo_root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--git-path")
        .arg("index")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let index_path = String::from_utf8(output.stdout).ok()?;
    let trimmed = index_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = PathBuf::from(trimmed);
    let index_path = if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    };
    Some(PathBuf::from(normalize_absolute_activity_path(&index_path)))
}
