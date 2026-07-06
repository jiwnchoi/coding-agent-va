use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::time::system_time_to_ms;

pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub(crate) fn list_jsonl_files(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }

    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .filter(|path| is_jsonl_file_name(path, None))
        .collect()
}

pub(crate) fn is_jsonl_file_name(path: &Path, prefix: Option<&str>) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            prefix.is_none_or(|prefix| name.starts_with(prefix)) && name.ends_with(".jsonl")
        })
}

pub(crate) fn file_updated_at_ms(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_to_ms)
        .unwrap_or_default()
}

pub(crate) fn file_stem_string(path: &Path) -> Option<String> {
    path.file_stem()?.to_str().map(str::to_string)
}

pub(crate) fn directory_title(path: &str) -> Option<String> {
    Path::new(path).file_name()?.to_str().map(str::to_string)
}

pub(crate) fn resolve_existing_dir(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("runtime home does not exist: {}", path.display()));
    }

    if !path.is_dir() {
        return Err(format!(
            "runtime home is not a directory: {}",
            path.display()
        ));
    }

    path.canonicalize()
        .map_err(|error| format!("failed to canonicalize runtime home: {error}"))
}
