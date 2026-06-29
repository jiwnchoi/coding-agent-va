# Toolchains

Last updated: 2026-06-29

## Required Project Tooling

This repository pins the required local CLI toolchain in `mise.toml`.

- Install project tools with `mise install`.
- Activate the environment with `mise trust` if mise asks for trust on first use.
- Use `just` recipes as the primary command entrypoint after tools are installed.

## Pinned Tooling

- `node` for the workspace runtime.
- The workspace package manager for dependency installation.
- `rust` with `rustfmt` and `clippy` for the Tauri backend.
- `just` for task orchestration.
- `lefthook` for git hook installation.

## Setup Flow

- `mise install`
- `just prepare`
- `just check`

## Notes

- `prepare-hooks` prefers a mise-provided `lefthook` binary and falls back to the local project binary when needed.
- Keep `mise.toml`, `package.json`, and `justfile` aligned when changing required tooling or setup workflow.
