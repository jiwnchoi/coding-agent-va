# Agent Session Protocols

Agent session support is implemented through a provider protocol layer in the Tauri backend.

## Supported Providers

- Codex: `~/.codex`, `sessions/**/rollout-*.jsonl`
- Claude Code: `~/.claude`, `projects/**/*.jsonl`
- Pi Agent: `~/.pi/agent`, `sessions/**/*.jsonl`

## Protocol Contract

Each provider implements the Rust `AgentSessionProtocol` trait in `src-tauri/src/agent_session.rs`.

A provider is responsible for:

- resolving its default runtime home
- listing sessions from runtime-specific transcript files
- deriving session title, cwd, transcript path, and update time
- producing watch targets for runtime artifacts
- filtering relevant file-system watch events
- extracting read, edited, and deleted file activity from its transcript format

Shared behavior stays outside provider implementations:

- git index watching
- working-tree diff loading
- git-status filtering for edited/deleted files
- impacted-file discovery through the workspace dependency indexer
- activity sorting and read-vs-edit deduplication

Session discovery first collects transcript paths and modification times without parsing transcript
contents. The combined provider list is sorted by recency, then metadata such as title and cwd is
hydrated in 20-session pages through Rayon. The frontend initially displays 10 of the first 20
hydrated sessions, advances in 10-session display batches, and fetches the next 20-session page as
the virtualized picker approaches the end of the hydrated data.
Watch planning reuses already hydrated sessions instead of forcing hydration of the full history,
and watcher registrations refresh when another page introduces a workspace.

Filesystem, Git, dependency-indexing, and transcript work invoked through Tauri commands runs on
Tokio's blocking pool; watcher event coordination remains on the asynchronous Tokio runtime.

## Node Descriptions

Codex node descriptions call the Codex Responses endpoint directly instead of spawning the Codex
CLI. The request replays model-visible `response_item` records from the selected rollout, honors
replacement history from compaction and thread rollback events, then appends the node-description
prompt as a new user message. Authentication is loaded from the selected Codex runtime home's
`auth.json`; expiring OAuth tokens are refreshed and written back to that file.
Requests also reuse the Codex client version from `models_cache.json` for the Codex `originator`
and `User-Agent` headers required by account-scoped model routing.

Claude Code and Pi descriptions continue to run through their CLIs with temporary or forked
sessions so description generation does not modify the selected source session.

## Adding a Provider

1. Add a variant to `AgentSessionProvider`.
2. Add a protocol struct implementing `AgentSessionProtocol`.
3. Register the protocol in `AgentSessionProvider::protocol()` and `AgentSessionProvider::all()`.
4. Add parser tests for session listing, file activity extraction, and watch path filtering.
5. Extend the TypeScript `AgentSessionProvider` union in `src/lib/session-watch.ts`.

## Frontend Model

The frontend consumes provider-neutral `AgentSessionSummary` records. Session IDs are namespaced as `<provider>:<providerSessionId>` so sessions from different agents can be shown in the same tabs and picker without collision.
