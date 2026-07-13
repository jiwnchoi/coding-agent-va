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
  - Walks a repository, builds repo/directory/file nodes, and invokes language-aware parsing for supported files.
- `symbol_extractor.rs`
  - Extracts top-level declarations such as functions, classes, structs, enums, interfaces, and similar symbols.
- `import_extractor.rs`
  - Extracts import-like edges using grammar-specific node kinds with a lightweight normalization pass.
- `graph.rs`
  - Defines the serializable graph payload returned over Tauri IPC.

## Concurrency model

- Tauri commands move repository scans and parsing onto Tokio's blocking pool so CPU and filesystem work does not occupy async runtime workers.
- Rayon parses supported source files in parallel, with one independent `tree-sitter` parser per file.
- Per-file results are merged after parallel parsing so graph construction stays lock-free and deterministic.
- Workspace dependency indexing uses the same parallel file-analysis model before performing its deterministic impact traversal.

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

- Add incremental file-level reindexing instead of full workspace walks.
- Improve import extraction for languages with more complex module syntax.
- Resolve internal imports to in-repo file targets when path resolution is available.
- Add optional language-specific semantic adapters when visualization needs more precise symbol linkage than `tree-sitter` alone can provide.
