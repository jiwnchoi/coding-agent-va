# Indexer

`src-tauri/src/indexer` is the Rust-side code graph indexer for the Tauri desktop app.

## Why it lives in Rust

- The desktop app already centralizes filesystem watching, SQLite reads, and rollout tailing in `src-tauri`.
- Code indexing has the same runtime profile: large repository walks, incremental refresh, and many parser invocations.
- The frontend should render graph state, not own parsing or repository traversal.

## Current architecture

- `language.rs`
  - Detects supported languages from file extensions and special filenames.
- `parser_registry.rs`
  - Maps supported languages to `tree-sitter` grammars.
- `workspace_indexer.rs`
  - Builds repo/directory/file nodes and the serialized architecture graph from a shared workspace index.
- `workspace_index.rs`
  - Owns the workspace-scoped in-memory index cache used by architecture graph and session impact queries.
  - Walks repositories while respecting `.gitignore` rules and excluding generated directories and nested Git worktrees.
  - Reuses unchanged parsed files and incrementally reparses changed files from their previous tree-sitter trees.
- `symbol_extractor.rs`
  - Extracts top-level declarations such as functions, classes, structs, enums, interfaces, and similar symbols.
- `import_extractor.rs`
  - Extracts import-like edges using grammar-specific node kinds with a lightweight normalization pass.
- `graph.rs`
  - Defines the serializable graph payload returned over Tauri IPC.

## Concurrency model

- Tauri commands move repository scans and parsing onto Tokio's blocking pool so CPU and filesystem work does not occupy async runtime workers.
- The Tauri-managed `WorkspaceIndexState` serializes refreshes for the same cache, so simultaneous graph and session-detail requests cannot duplicate a cold workspace parse.
- Rayon parses supported source files in parallel, with one independent `tree-sitter` parser per file.
- Cached file fingerprints reuse unchanged parse trees and extracted symbols, imports, and identifiers.
- Changed files apply a tree-sitter `InputEdit` to the previous tree and use it as the incremental parse input.
- The cache retains the four most recently used workspaces to bound source and tree memory.
- Workspace dependency traversal and architecture graph construction share the refreshed snapshot and remain deterministic.

## Cache lifecycle

- The cache is process-local and survives navigation between sessions for the lifetime of the desktop application.
- Every graph or impact query refreshes the workspace snapshot by comparing file length and modification time.
- Deleted files are removed, new files are parsed, and only changed supported files are incrementally reparsed.
- A desktop application restart starts with a cold cache because tree-sitter trees are runtime objects and are not serialized to disk.

## Supported languages

Current parser coverage includes:

- TypeScript / TSX
- JavaScript / JSX
- Rust
- Python
- Go
- Java
- C / C++
- C#
- PHP
- Ruby
- Swift
- Kotlin
- Bash
- JSON
- YAML
- TOML
- HTML
- CSS

Coverage means:

- The indexer can detect the language.
- The indexer can parse the file with a `tree-sitter` grammar.
- The indexer can attach file-level metadata to the graph.

Symbol and import extraction are intentionally heuristic in the first pass. The goal is broad, stable, multi-language structure extraction for visualization, not perfect semantic indexing.

## Graph contract

The indexer currently emits:

- `repo` nodes
- `directory` nodes
- `file` nodes
- `symbol` nodes
- `external` nodes

And these edge kinds:

- `contains`
- `declares`
- `imports`

## Tauri interface

The Rust backend exposes two commands:

- `list_indexer_languages`
- `index_workspace_graph`

These commands let the React UI query supported languages and request a fresh graph snapshot for a workspace path.

## Next steps

- Replace metadata refresh scans with a dedicated workspace watcher if repository traversal becomes measurable for exceptionally large workspaces.
- Improve import extraction for languages with more complex module syntax.
- Resolve internal imports to in-repo file targets when path resolution is available.
- Add optional language-specific semantic adapters when visualization needs more precise symbol linkage than `tree-sitter` alone can provide.
