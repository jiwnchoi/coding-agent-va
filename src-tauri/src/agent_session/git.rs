use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::file_system::file_updated_at_ms;
use super::paths::normalize_absolute_activity_path;
use super::types::{AgentSessionFileDiff, AgentSessionSummary};

#[derive(Default)]
pub(crate) struct GitWorktreeStatus {
    pub(crate) edited_files: HashSet<String>,
    pub(crate) deleted_files: HashSet<String>,
}

pub(crate) fn read_git_worktree_status(
    candidate_paths: &HashSet<String>,
) -> Option<GitWorktreeStatus> {
    if candidate_paths.is_empty() {
        return Some(GitWorktreeStatus::default());
    }

    let mut relative_paths_by_repo_root = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
    for candidate_path in candidate_paths {
        let candidate_path = Path::new(candidate_path);
        let Some(repo_root) = resolve_git_repo_root_for_activity_path(candidate_path) else {
            continue;
        };
        let Ok(relative_path) = candidate_path.strip_prefix(&repo_root) else {
            continue;
        };

        relative_paths_by_repo_root
            .entry(repo_root)
            .or_default()
            .insert(relative_path.to_path_buf());
    }
    if relative_paths_by_repo_root.is_empty() {
        return None;
    }

    let mut status = GitWorktreeStatus::default();
    for (repo_root, relative_paths) in relative_paths_by_repo_root {
        let output = read_git_status_for_paths(&repo_root, relative_paths)?;
        append_git_status_entries(&mut status, &repo_root, &output.stdout);
    }

    Some(status)
}

fn read_git_status_for_paths(
    repo_root: &Path,
    relative_paths: BTreeSet<PathBuf>,
) -> Option<std::process::Output> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_root)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .arg("--");
    command.args(relative_paths);
    let output = command.output().ok()?;

    output.status.success().then_some(output)
}

fn append_git_status_entries(status: &mut GitWorktreeStatus, repo_root: &Path, stdout: &[u8]) {
    let mut entries = stdout.split(|byte| *byte == 0);
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }

        let index_status = entry[0] as char;
        let worktree_status = entry[1] as char;
        let path = String::from_utf8_lossy(&entry[3..]).to_string();
        if path.is_empty() {
            continue;
        }

        if matches!(index_status, 'R' | 'C') {
            let _ = entries.next();
        }

        let absolute_path = normalize_absolute_activity_path(&repo_root.join(path));
        if index_status == 'D' || worktree_status == 'D' {
            status.deleted_files.insert(absolute_path);
        } else {
            status.edited_files.insert(absolute_path);
        }
    }
}

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

pub(crate) fn read_git_index_updated_at_ms(cwd: &str) -> Option<u64> {
    let repo_root = resolve_git_repo_root_from_path(Path::new(cwd))?;
    let index_path = resolve_git_index_path(&repo_root)?;

    Some(file_updated_at_ms(&index_path))
}

pub(crate) fn collect_git_index_paths_from_sessions(
    sessions: &[AgentSessionSummary],
) -> Vec<PathBuf> {
    let mut cwd_paths = BTreeSet::<PathBuf>::new();
    let mut repo_roots = BTreeSet::<PathBuf>::new();

    for session in sessions {
        let Some(cwd) = &session.cwd else {
            continue;
        };

        cwd_paths.insert(PathBuf::from(cwd));
    }

    for cwd in cwd_paths {
        let Some(repo_root) = resolve_git_repo_root_from_path(&cwd) else {
            continue;
        };

        repo_roots.insert(repo_root);
    }

    repo_roots
        .into_iter()
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

fn resolve_git_repo_root_for_activity_path(activity_path: &Path) -> Option<PathBuf> {
    let search_start = if activity_path.is_dir() {
        activity_path
    } else {
        activity_path.parent()?
    };

    search_start
        .ancestors()
        .filter(|ancestor| ancestor.exists())
        .find_map(resolve_git_repo_root_from_path)
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
