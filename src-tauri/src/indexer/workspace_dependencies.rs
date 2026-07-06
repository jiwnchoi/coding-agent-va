use super::import_extractor::extract_imports;
use super::language::{detect_language, SupportedLanguage};
use super::parser_registry::parse_source;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

#[cfg(test)]
pub fn find_impacted_files(
    workspace_root: &Path,
    changed_files: &[PathBuf],
) -> Result<Vec<String>, String> {
    let impacted_relations = find_impacted_file_relations(workspace_root, changed_files)?;

    Ok(impacted_relations
        .into_iter()
        .map(|relation| relation.impacted_file)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ImpactedFileRelation {
    pub changed_file: String,
    pub impacted_file: String,
    pub import_specifier: String,
}

pub fn find_impacted_file_relations(
    workspace_root: &Path,
    changed_files: &[PathBuf],
) -> Result<Vec<ImpactedFileRelation>, String> {
    if !workspace_root.exists() {
        return Ok(Vec::new());
    }

    let workspace_root = workspace_root
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize workspace path: {error}"))?;
    let changed_files = changed_files
        .iter()
        .map(|path| normalize_path(path, &workspace_root))
        .collect::<HashSet<_>>();
    let mut known_files = collect_known_files(&workspace_root)?;
    known_files.extend(changed_files.iter().cloned());

    let mut impacted_relations = BTreeSet::<ImpactedFileRelation>::new();

    for importer in &known_files {
        if !importer.is_file() {
            continue;
        }

        let Some(language) = detect_language(importer) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(importer) else {
            continue;
        };
        let Ok(tree) = parse_source(language, &source) else {
            continue;
        };

        for import in extract_imports(language, &source, tree.root_node()) {
            for resolved in resolve_internal_import(
                importer,
                &import.specifier,
                language,
                &workspace_root,
                &known_files,
            ) {
                if !changed_files.contains(&resolved) || changed_files.contains(importer) {
                    continue;
                }

                let Some(changed_file) = relative_workspace_path(&resolved, &workspace_root) else {
                    continue;
                };
                let Some(impacted_file) = relative_workspace_path(importer, &workspace_root) else {
                    continue;
                };

                impacted_relations.insert(ImpactedFileRelation {
                    changed_file,
                    impacted_file,
                    import_specifier: import.specifier.clone(),
                });
            }
        }
    }

    Ok(impacted_relations.into_iter().collect())
}

fn collect_known_files(workspace_root: &Path) -> Result<HashSet<PathBuf>, String> {
    let mut known_files = HashSet::new();

    for entry in WalkDir::new(workspace_root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry.map_err(|error| format!("failed to walk workspace: {error}"))?;
        if entry.file_type().is_file() {
            known_files.insert(normalize_path(entry.path(), workspace_root));
        }
    }

    Ok(known_files)
}

fn resolve_internal_import(
    importer: &Path,
    specifier: &str,
    language: SupportedLanguage,
    workspace_root: &Path,
    known_files: &HashSet<PathBuf>,
) -> Vec<PathBuf> {
    if specifier.is_empty() || is_external_specifier(specifier) {
        return Vec::new();
    }

    let base_directory = importer.parent().unwrap_or(workspace_root);
    let base_path = if specifier.starts_with('/') {
        workspace_root.join(specifier.trim_start_matches('/'))
    } else {
        base_directory.join(specifier)
    };

    candidate_paths(&base_path, language)
        .into_iter()
        .map(|candidate| normalize_path(&candidate, workspace_root))
        .filter(|candidate| known_files.contains(candidate))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn candidate_paths(base_path: &Path, language: SupportedLanguage) -> Vec<PathBuf> {
    let mut candidates = vec![base_path.to_path_buf()];

    if base_path.extension().is_none() {
        for extension in language_extensions(language) {
            candidates.push(base_path.with_extension(extension));
        }

        for index_name in index_file_names(language) {
            candidates.push(base_path.join(index_name));
        }
    }

    candidates
}

fn language_extensions(language: SupportedLanguage) -> &'static [&'static str] {
    match language {
        SupportedLanguage::JavaScript => &["js", "mjs", "cjs", "jsx", "ts", "tsx"],
        SupportedLanguage::Jsx => &["jsx", "js", "tsx", "ts"],
        SupportedLanguage::TypeScript => &["ts", "tsx", "js", "jsx", "mts", "cts"],
        SupportedLanguage::Tsx => &["tsx", "ts", "jsx", "js"],
        SupportedLanguage::Json => &["json"],
        SupportedLanguage::Css => &["css"],
        SupportedLanguage::Html => &["html", "htm"],
        SupportedLanguage::Python => &["py"],
        SupportedLanguage::Ruby => &["rb"],
        SupportedLanguage::Php => &["php", "phtml"],
        SupportedLanguage::Swift => &["swift"],
        SupportedLanguage::Kotlin => &["kt", "kts"],
        SupportedLanguage::Java => &["java"],
        SupportedLanguage::Go => &["go"],
        SupportedLanguage::Rust => &["rs"],
        SupportedLanguage::C => &["c", "h"],
        SupportedLanguage::Cpp => &["cc", "cpp", "cxx", "hh", "hpp", "hxx"],
        SupportedLanguage::CSharp => &["cs"],
        SupportedLanguage::Bash => &["sh", "bash"],
        SupportedLanguage::Yaml => &["yaml", "yml"],
        SupportedLanguage::Toml => &["toml"],
    }
}

fn index_file_names(language: SupportedLanguage) -> Vec<String> {
    language_extensions(language)
        .iter()
        .map(|extension| format!("index.{extension}"))
        .collect()
}

fn is_external_specifier(specifier: &str) -> bool {
    !(specifier.starts_with('.') || specifier.starts_with('/'))
}

fn normalize_path(path: &Path, workspace_root: &Path) -> PathBuf {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };

    absolute_path
        .canonicalize()
        .unwrap_or_else(|_| normalize_missing_path(&absolute_path))
}

fn normalize_missing_path(path: &Path) -> PathBuf {
    let mut suffix = Vec::new();
    let mut existing_ancestor = Some(path);

    while let Some(ancestor) = existing_ancestor {
        if ancestor.exists() {
            let mut normalized = ancestor
                .canonicalize()
                .unwrap_or_else(|_| ancestor.to_path_buf());
            for component in suffix.iter().rev() {
                normalized.push(component);
            }
            return normalized;
        }

        if let Some(file_name) = ancestor.file_name() {
            suffix.push(file_name.to_os_string());
        }
        existing_ancestor = ancestor.parent();
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn relative_workspace_path(path: &Path, workspace_root: &Path) -> Option<String> {
    path.strip_prefix(workspace_root)
        .ok()
        .map(|path| path.display().to_string())
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".turbo")
        )
    })
}
