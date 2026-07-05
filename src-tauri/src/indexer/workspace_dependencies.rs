use super::import_extractor::extract_imports;
use super::language::{detect_language, SupportedLanguage};
use super::parser_registry::parse_source;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

pub fn find_impacted_files(
    workspace_root: &Path,
    changed_files: &[PathBuf],
) -> Result<Vec<String>, String> {
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

    let mut reverse_dependencies = HashMap::<PathBuf, BTreeSet<PathBuf>>::new();

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
                reverse_dependencies
                    .entry(resolved)
                    .or_default()
                    .insert(importer.clone());
            }
        }
    }

    let impacted_files = changed_files
        .iter()
        .flat_map(|changed_file| reverse_dependencies.get(changed_file).into_iter().flatten())
        .filter(|importer| !changed_files.contains(*importer))
        .filter_map(|importer| {
            importer
                .strip_prefix(&workspace_root)
                .ok()
                .map(|path| path.display().to_string())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(impacted_files)
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

fn is_ignored(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".turbo")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::find_impacted_files;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finds_relative_importers_of_changed_file() {
        let workspace_root = create_temp_workspace("changed");
        let src_dir = workspace_root.join("src");
        fs::create_dir_all(&src_dir).expect("create src dir");
        fs::write(
            src_dir.join("app.ts"),
            "import { value } from './dep';\nconsole.log(value);\n",
        )
        .expect("write app");
        fs::write(src_dir.join("dep.ts"), "export const value = 1;\n").expect("write dep");

        let impacted_files =
            find_impacted_files(&workspace_root, &[src_dir.join("dep.ts")]).expect("index deps");

        assert_eq!(impacted_files, vec!["src/app.ts"]);
        fs::remove_dir_all(workspace_root).expect("cleanup workspace");
    }

    #[test]
    fn finds_importers_of_deleted_file_when_path_is_missing() {
        let workspace_root = create_temp_workspace("deleted");
        let src_dir = workspace_root.join("src");
        fs::create_dir_all(&src_dir).expect("create src dir");
        fs::write(
            src_dir.join("app.ts"),
            "import { missingValue } from './deleted';\nconsole.log(missingValue);\n",
        )
        .expect("write app");

        let impacted_files = find_impacted_files(&workspace_root, &[src_dir.join("deleted.ts")])
            .expect("index deps");

        assert_eq!(impacted_files, vec!["src/app.ts"]);
        fs::remove_dir_all(workspace_root).expect("cleanup workspace");
    }

    fn create_temp_workspace(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time")
            .as_nanos();
        let workspace_root = std::env::temp_dir().join(format!("coding-agent-va-{label}-{unique}"));
        fs::create_dir_all(&workspace_root).expect("create temp workspace");
        workspace_root
    }
}
