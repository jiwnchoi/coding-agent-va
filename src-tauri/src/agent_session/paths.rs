use std::path::{Path, PathBuf};

pub(crate) fn normalize_written_activity_path(path: &str, cwd: Option<&str>) -> Option<String> {
    let path = Path::new(path);
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let cwd = PathBuf::from(cwd?);
        cwd.canonicalize().unwrap_or(cwd).join(path)
    };

    Some(normalize_absolute_activity_path(&absolute_path))
}

pub(crate) fn normalize_absolute_activity_path(path: &Path) -> String {
    if let Ok(canonical_path) = path.canonicalize() {
        return canonical_path.display().to_string();
    }

    path.parent()
        .and_then(|parent| {
            let canonical_parent = parent.canonicalize().ok()?;
            let file_name = path.file_name()?;
            Some(canonical_parent.join(file_name).display().to_string())
        })
        .unwrap_or_else(|| path.display().to_string())
}
