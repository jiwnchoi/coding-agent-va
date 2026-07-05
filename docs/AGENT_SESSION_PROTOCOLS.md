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

## Adding a Provider

1. Add a variant to `AgentSessionProvider`.
2. Add a protocol struct implementing `AgentSessionProtocol`.
3. Register the protocol in `AgentSessionProvider::protocol()` and `AgentSessionProvider::all()`.
4. Add parser tests for session listing, file activity extraction, and watch path filtering.
5. Extend the TypeScript `AgentSessionProvider` union in `src/lib/session-watch.ts`.

## Frontend Model

The frontend consumes provider-neutral `AgentSessionSummary` records. Session IDs are namespaced as `<provider>:<providerSessionId>` so sessions from different agents can be shown in the same tabs and picker without collision.
