use super::graph::{ArchitectureEdge, ArchitectureGraph, ArchitectureNode, EdgeKind, NodeKind};
use super::import_extractor::{extract_imports, ExtractedImport};
use super::language::{detect_language, SupportedLanguage};
use super::parser_registry::parse_source;
use super::symbol_extractor::{extract_symbols, ExtractedSymbol};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct WorkspaceIndexer;

impl WorkspaceIndexer {
    pub fn index(workspace_path: &Path) -> Result<ArchitectureGraph, String> {
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

        let workspace_path = workspace_path
            .canonicalize()
            .map_err(|error| format!("failed to canonicalize workspace path: {error}"))?;

        let mut graph = ArchitectureGraph::default();
        let mut seen_nodes = HashSet::new();
        let mut seen_edges = HashSet::new();
        let mut external_nodes = HashMap::<String, String>::new();
        let workspace_id = node_id(&workspace_path);

        push_node(
            &mut graph,
            &mut seen_nodes,
            ArchitectureNode {
                id: workspace_id.clone(),
                kind: NodeKind::Repo,
                label: workspace_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
                path: Some(workspace_path.display().to_string()),
                metadata: BTreeMap::new(),
            },
        );

        let entries = WalkDir::new(&workspace_path)
            .into_iter()
            .filter_entry(|entry| !is_ignored(entry.path()))
            .filter_map(Result::ok)
            .filter(|entry| entry.path() != workspace_path)
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        let indexed_files = entries
            .par_iter()
            .filter(|path| path.is_file())
            .filter_map(|path| index_file(path))
            .collect::<HashMap<_, _>>();

        for path in entries {
            let path = path.as_path();
            if path == workspace_path {
                continue;
            }

            let node_kind = if path.is_dir() {
                NodeKind::Directory
            } else {
                NodeKind::File
            };

            let node = ArchitectureNode {
                id: node_id(path),
                kind: node_kind,
                label: path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_string(),
                path: Some(path.display().to_string()),
                metadata: node_metadata(path, detect_language(path)),
            };
            let node_id_value = node.id.clone();
            push_node(&mut graph, &mut seen_nodes, node);

            if let Some(parent) = path.parent() {
                let parent_id = if parent == workspace_path {
                    workspace_id.clone()
                } else {
                    node_id(parent)
                };
                push_edge(
                    &mut graph,
                    &mut seen_edges,
                    ArchitectureEdge {
                        id: edge_id(EdgeKind::Contains, &parent_id, &node_id_value, None),
                        kind: EdgeKind::Contains,
                        source: parent_id,
                        target: node_id_value.clone(),
                        label: None,
                    },
                );
            }

            if let Some(indexed_file) = indexed_files.get(path) {
                merge_indexed_file(
                    &mut graph,
                    &mut seen_nodes,
                    &mut seen_edges,
                    &mut external_nodes,
                    path,
                    &node_id_value,
                    indexed_file,
                );
            }
        }

        Ok(graph)
    }
}

struct IndexedFile {
    language: SupportedLanguage,
    symbols: Vec<ExtractedSymbol>,
    imports: Vec<ExtractedImport>,
}

fn index_file(path: &Path) -> Option<(PathBuf, IndexedFile)> {
    let language = detect_language(path)?;
    let source = fs::read_to_string(path).ok()?;
    let tree = parse_source(language, &source).ok()?;
    let root = tree.root_node();

    Some((
        path.to_path_buf(),
        IndexedFile {
            language,
            symbols: extract_symbols(language, &source, root),
            imports: extract_imports(language, &source, root),
        },
    ))
}

fn merge_indexed_file(
    graph: &mut ArchitectureGraph,
    seen_nodes: &mut HashSet<String>,
    seen_edges: &mut HashSet<String>,
    external_nodes: &mut HashMap<String, String>,
    path: &Path,
    file_node_id: &str,
    indexed_file: &IndexedFile,
) {
    for symbol in &indexed_file.symbols {
        let symbol_node_id = format!("{file_node_id}#symbol:{}", symbol.name);
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "language".to_string(),
            indexed_file.language.as_str().to_string(),
        );
        metadata.insert("symbol_kind".to_string(), symbol.kind.clone());

        push_node(
            graph,
            seen_nodes,
            ArchitectureNode {
                id: symbol_node_id.clone(),
                kind: NodeKind::Symbol,
                label: symbol.name.clone(),
                path: Some(path.display().to_string()),
                metadata,
            },
        );

        push_edge(
            graph,
            seen_edges,
            ArchitectureEdge {
                id: edge_id(EdgeKind::Declares, file_node_id, &symbol_node_id, None),
                kind: EdgeKind::Declares,
                source: file_node_id.to_string(),
                target: symbol_node_id,
                label: None,
            },
        );
    }

    for import in &indexed_file.imports {
        let external_node_id = external_nodes
            .entry(import.specifier.clone())
            .or_insert_with(|| format!("external:{}", import.specifier))
            .clone();

        if !seen_nodes.contains(&external_node_id) {
            let mut metadata = BTreeMap::new();
            metadata.insert("specifier".to_string(), import.specifier.clone());

            push_node(
                graph,
                seen_nodes,
                ArchitectureNode {
                    id: external_node_id.clone(),
                    kind: NodeKind::External,
                    label: import.specifier.clone(),
                    path: None,
                    metadata,
                },
            );
        }

        push_edge(
            graph,
            seen_edges,
            ArchitectureEdge {
                id: edge_id(
                    EdgeKind::Imports,
                    file_node_id,
                    &external_node_id,
                    Some(&import.specifier),
                ),
                kind: EdgeKind::Imports,
                source: file_node_id.to_string(),
                target: external_node_id,
                label: Some(import.specifier.clone()),
            },
        );
    }
}

fn push_node(graph: &mut ArchitectureGraph, seen: &mut HashSet<String>, node: ArchitectureNode) {
    if seen.insert(node.id.clone()) {
        graph.nodes.push(node);
    }
}

fn push_edge(graph: &mut ArchitectureGraph, seen: &mut HashSet<String>, edge: ArchitectureEdge) {
    if seen.insert(edge.id.clone()) {
        graph.edges.push(edge);
    }
}

fn node_id(path: &Path) -> String {
    format!("path:{}", path.display())
}

fn edge_id(kind: EdgeKind, source: &str, target: &str, label: Option<&str>) -> String {
    let kind = match kind {
        EdgeKind::Contains => "contains",
        EdgeKind::Imports => "imports",
        EdgeKind::Declares => "declares",
    };
    match label {
        Some(label) => format!("{kind}:{source}->{target}:{label}"),
        None => format!("{kind}:{source}->{target}"),
    }
}

fn node_metadata(path: &Path, language: Option<SupportedLanguage>) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    if let Some(language) = language {
        metadata.insert("language".to_string(), language.as_str().to_string());
    }
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        metadata.insert("extension".to_string(), extension.to_string());
    }
    metadata
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
    use super::WorkspaceIndexer;
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn indexes_workspace_shape_and_symbols() {
        let temp_dir = env::temp_dir().join(format!(
            "coding-agent-va-indexer-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix timestamp")
                .as_nanos()
        ));
        let source_dir = temp_dir.join("src");
        fs::create_dir_all(&source_dir).expect("create src");
        fs::write(
            source_dir.join("main.ts"),
            "import { foo } from './dep';\nexport function run() { return foo; }\n",
        )
        .expect("write main");

        let graph = WorkspaceIndexer::index(&temp_dir).expect("index workspace");
        assert!(graph.nodes.iter().any(|node| node.label == "main.ts"));
        assert!(graph.nodes.iter().any(|node| node.label == "run"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.label.as_deref() == Some("./dep")));
        fs::remove_dir_all(&temp_dir).expect("cleanup temp dir");
    }
}
