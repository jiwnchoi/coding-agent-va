use super::graph::{ArchitectureEdge, ArchitectureGraph, ArchitectureNode, EdgeKind, NodeKind};
use super::language::{detect_language, SupportedLanguage};
#[cfg(test)]
use super::workspace_index::WorkspaceIndexState;
use super::workspace_index::{IndexedSource, WorkspaceIndex};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

pub struct WorkspaceIndexer;

impl WorkspaceIndexer {
    #[cfg(test)]
    pub fn index(workspace_path: &Path) -> Result<ArchitectureGraph, String> {
        let state = WorkspaceIndexState::default();
        let index = state.snapshot(workspace_path)?;
        Ok(Self::index_cached(&index))
    }

    pub(crate) fn index_cached(index: &WorkspaceIndex) -> ArchitectureGraph {
        let workspace_path = &index.root;

        let mut graph = ArchitectureGraph::default();
        let mut seen_nodes = HashSet::new();
        let mut seen_edges = HashSet::new();
        let mut external_nodes = HashMap::<String, String>::new();
        let workspace_id = node_id(workspace_path);

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

        for path in &index.entries {
            let path = path.as_path();
            if path == index.root {
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

            if let Some(indexed_file) = index.files.get(path) {
                merge_indexed_file(
                    &mut graph,
                    &mut seen_nodes,
                    &mut seen_edges,
                    &mut external_nodes,
                    path,
                    &node_id_value,
                    indexed_file.as_ref(),
                );
            }
        }

        graph
    }
}

fn merge_indexed_file(
    graph: &mut ArchitectureGraph,
    seen_nodes: &mut HashSet<String>,
    seen_edges: &mut HashSet<String>,
    external_nodes: &mut HashMap<String, String>,
    path: &Path,
    file_node_id: &str,
    indexed_file: &IndexedSource,
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

const IGNORED_DIRECTORY_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
];

pub(super) fn is_ignored(path: &Path, workspace_root: &Path) -> bool {
    if path == workspace_root {
        return false;
    }

    let Ok(relative_path) = path.strip_prefix(workspace_root) else {
        return true;
    };

    relative_path.components().any(|component| {
        IGNORED_DIRECTORY_NAMES.contains(&component.as_os_str().to_str().unwrap_or_default())
    }) || (path.is_dir() && path.join(".git").exists())
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

    #[test]
    fn excludes_nested_git_worktrees() {
        let temp_dir = env::temp_dir().join(format!(
            "coding-agent-va-nested-worktree-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix timestamp")
                .as_nanos()
        ));
        let current_src = temp_dir.join("src");
        let nested_worktree = temp_dir.join(".claude/worktrees/feature");
        fs::create_dir_all(&current_src).expect("create current worktree source");
        fs::create_dir_all(nested_worktree.join("src")).expect("create nested worktree source");
        fs::write(
            current_src.join("current.ts"),
            "export const current = true;\n",
        )
        .expect("write current source");
        fs::write(
            nested_worktree.join(".git"),
            "gitdir: /repo/.git/worktrees/feature\n",
        )
        .expect("write worktree git marker");
        fs::write(
            nested_worktree.join("src/nested.ts"),
            "export const nested = true;\n",
        )
        .expect("write nested source");

        let graph = WorkspaceIndexer::index(&temp_dir).expect("index workspace");
        assert!(graph.nodes.iter().any(|node| node.label == "current.ts"));
        assert!(!graph.nodes.iter().any(|node| node.label == "feature"));
        assert!(!graph.nodes.iter().any(|node| node.label == "nested.ts"));
        fs::remove_dir_all(&temp_dir).expect("cleanup temp dir");
    }
}
