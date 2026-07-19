use super::import_extractor::{extract_code_identifiers, extract_imports, ExtractedImport};
use super::language::{detect_language, SupportedLanguage};
use super::parser_registry::{parse_source, parse_source_incrementally};
use super::symbol_extractor::{extract_symbols, ExtractedSymbol};
use super::workspace_indexer::is_ignored;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tree_sitter::Tree;

#[derive(Clone, Default)]
pub struct WorkspaceIndexState {
    cache: Arc<Mutex<WorkspaceIndexCache>>,
}

const MAX_CACHED_WORKSPACES: usize = 4;

#[derive(Default)]
struct WorkspaceIndexCache {
    indexes: HashMap<PathBuf, Arc<WorkspaceIndex>>,
    access_order: VecDeque<PathBuf>,
}

impl WorkspaceIndexState {
    pub(crate) fn snapshot(&self, workspace_path: &Path) -> Result<Arc<WorkspaceIndex>, String> {
        let workspace_root = canonical_workspace_root(workspace_path)?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| "failed to lock workspace index cache".to_string())?;
        let previous = cache.indexes.get(&workspace_root).cloned();
        let refreshed = Arc::new(WorkspaceIndex::refresh(
            &workspace_root,
            previous.as_deref(),
        )?);
        cache
            .indexes
            .insert(workspace_root.clone(), refreshed.clone());
        cache.access_order.retain(|path| path != &workspace_root);
        cache.access_order.push_back(workspace_root);
        while cache.indexes.len() > MAX_CACHED_WORKSPACES {
            if let Some(oldest) = cache.access_order.pop_front() {
                cache.indexes.remove(&oldest);
            }
        }
        Ok(refreshed)
    }
}

pub(crate) struct WorkspaceIndex {
    pub(crate) root: PathBuf,
    pub(crate) entries: Vec<PathBuf>,
    pub(crate) files: HashMap<PathBuf, Arc<IndexedSource>>,
}

impl WorkspaceIndex {
    fn refresh(workspace_root: &Path, previous: Option<&Self>) -> Result<Self, String> {
        let entries = collect_entries(workspace_root)?;
        let fingerprints = entries
            .par_iter()
            .filter(|path| path.is_file())
            .filter_map(|path| {
                detect_language(path)?;
                FileFingerprint::read(path).map(|fingerprint| (path.clone(), fingerprint))
            })
            .collect::<HashMap<_, _>>();
        let files = fingerprints
            .par_iter()
            .filter_map(|(path, fingerprint)| {
                let previous_file = previous.and_then(|index| index.files.get(path));
                if previous_file.is_some_and(|file| file.fingerprint == *fingerprint) {
                    return previous_file.cloned().map(|file| (path.clone(), file));
                }
                index_source(path, *fingerprint, previous_file.map(Arc::as_ref))
                    .map(|file| (path.clone(), Arc::new(file)))
            })
            .collect();

        Ok(Self {
            root: workspace_root.to_path_buf(),
            entries,
            files,
        })
    }
}

pub(crate) struct IndexedSource {
    pub(crate) language: SupportedLanguage,
    pub(crate) source: String,
    pub(crate) tree: Tree,
    pub(crate) symbols: Vec<ExtractedSymbol>,
    pub(crate) imports: Vec<ExtractedImport>,
    pub(crate) code_identifiers: std::collections::HashSet<String>,
    fingerprint: FileFingerprint,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FileFingerprint {
    len: u64,
    modified: SystemTime,
}

impl FileFingerprint {
    fn read(path: &Path) -> Option<Self> {
        let metadata = path.metadata().ok()?;
        Some(Self {
            len: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}

fn canonical_workspace_root(workspace_path: &Path) -> Result<PathBuf, String> {
    if !workspace_path.exists() {
        return Err(format!(
            "workspace path does not exist: {}",
            workspace_path.display()
        ));
    }
    if !workspace_path.is_dir() {
        return Err(format!(
            "workspace path is not a directory: {}",
            workspace_path.display()
        ));
    }
    workspace_path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize workspace path: {error}"))
}

fn collect_entries(workspace_root: &Path) -> Result<Vec<PathBuf>, String> {
    let filter_root = workspace_root.to_path_buf();
    let mut builder = WalkBuilder::new(workspace_root);
    builder
        .hidden(false)
        .ignore(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .filter_entry(move |entry| !is_ignored(entry.path(), &filter_root));

    builder
        .build()
        .filter_map(|entry| match entry {
            Ok(entry) if entry.path() != workspace_root => Some(Ok(entry.into_path())),
            Ok(_) => None,
            Err(error) => Some(Err(format!("failed to walk workspace: {error}"))),
        })
        .collect()
}

fn index_source(
    path: &Path,
    fingerprint: FileFingerprint,
    previous: Option<&IndexedSource>,
) -> Option<IndexedSource> {
    let language = detect_language(path)?;
    let source = fs::read_to_string(path).ok()?;
    let tree = previous
        .filter(|previous| previous.language == language)
        .and_then(|previous| {
            parse_source_incrementally(language, &previous.source, &source, &previous.tree).ok()
        })
        .or_else(|| parse_source(language, &source).ok())?;
    let root = tree.root_node();
    let imports = extract_imports(language, &source, root);
    let code_identifiers = if imports.is_empty() {
        std::collections::HashSet::new()
    } else {
        extract_code_identifiers(language, &source, root)
    };

    Some(IndexedSource {
        language,
        symbols: extract_symbols(language, &source, root),
        imports,
        code_identifiers,
        source,
        tree,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::WorkspaceIndexState;
    use std::fs;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reuses_unchanged_files_and_reindexes_only_changed_files() {
        let root = std::env::temp_dir().join(format!(
            "coding-agent-va-index-cache-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create workspace");
        let stable_path = root.join("stable.ts");
        let changed_path = root.join("changed.ts");
        fs::write(&stable_path, "export const stable = true;\n").expect("write stable file");
        fs::write(&changed_path, "import './before';\n").expect("write changed file");
        let stable_index_path = stable_path.canonicalize().expect("canonical stable path");
        let changed_index_path = changed_path.canonicalize().expect("canonical changed path");
        let state = WorkspaceIndexState::default();

        let first = state.snapshot(&root).expect("first snapshot");
        fs::write(&changed_path, "import './after-longer';\n").expect("update changed file");
        let second = state.snapshot(&root).expect("second snapshot");

        assert!(Arc::ptr_eq(
            first
                .files
                .get(&stable_index_path)
                .expect("first stable file"),
            second
                .files
                .get(&stable_index_path)
                .expect("second stable file")
        ));
        assert!(!Arc::ptr_eq(
            first
                .files
                .get(&changed_index_path)
                .expect("first changed file"),
            second
                .files
                .get(&changed_index_path)
                .expect("second changed file")
        ));
        assert_eq!(
            second.files[&changed_index_path].imports[0].specifier,
            "./after-longer"
        );

        fs::remove_dir_all(root).expect("cleanup workspace");
    }

    #[test]
    fn excludes_gitignored_files_and_keeps_negated_files() {
        let root = std::env::temp_dir().join(format!(
            "coding-agent-va-gitignore-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let generated = root.join("generated");
        fs::create_dir_all(&generated).expect("create generated directory");
        fs::write(
            root.join(".gitignore"),
            "ignored.ts\ngenerated/*\n!generated/keep.ts\n",
        )
        .expect("write gitignore");
        fs::write(root.join("visible.ts"), "export const visible = true;\n")
            .expect("write visible source");
        fs::write(root.join("ignored.ts"), "export const ignored = true;\n")
            .expect("write ignored source");
        fs::write(
            generated.join("ignored.ts"),
            "export const generatedIgnored = true;\n",
        )
        .expect("write generated ignored source");
        fs::write(
            generated.join("keep.ts"),
            "export const generatedKeep = true;\n",
        )
        .expect("write generated kept source");

        let index = WorkspaceIndexState::default()
            .snapshot(&root)
            .expect("index workspace");
        let indexed_root = root.canonicalize().expect("canonical workspace path");
        let indexed_generated = indexed_root.join("generated");

        assert!(index.entries.contains(&indexed_root.join("visible.ts")));
        assert!(index.entries.contains(&indexed_generated.join("keep.ts")));
        assert!(!index.entries.contains(&indexed_root.join("ignored.ts")));
        assert!(!index
            .entries
            .contains(&indexed_generated.join("ignored.ts")));

        fs::remove_dir_all(root).expect("cleanup workspace");
    }
}
