# coding-agent-va

## Development Setup

### Required Tools

This project pins local development tool versions in `mise.toml`.

- `node` `24.8.0`
- `pnpm` `11.9.0`
- `rust` `1.92.0` (with `rustfmt` and `clippy`)
- `just` `1.43.1`
- `lefthook` `2.1.9`

### Initial Setup

```bash
mise trust
mise install
just prepare
```

### Validation

```bash
just check
```

## Agent Session Providers

The desktop app can inspect Codex, Claude Code, and Pi Agent session transcripts through a provider protocol layer.

- See `docs/AGENT_SESSION_PROTOCOLS.md` for the backend protocol contract and extension steps.

### Common Commands

```bash
just --list
just build
just test
```
