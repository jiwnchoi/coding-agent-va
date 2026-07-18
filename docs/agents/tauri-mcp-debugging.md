# Tauri MCP Debugging

## Purpose

Use the project-local Tauri MCP server to inspect and operate the running desktop application directly. This is an exploratory debugging workflow: the agent keeps a live connection, reads the DOM and logs, interacts with the UI, edits code, and reconnects after the app restarts.

The MCP server configuration lives in `.codex/config.toml`. Codex loads project-scoped MCP servers only after the repository is trusted. Restart Codex after changing MCP configuration, then use `/mcp` to verify that the `tauri` server is available.

## Start the Application

Run the MCP-enabled debug application in a persistent terminal:

```bash
just dev-mcp
```

The MCP bridge is registered only when `TAURI_MCP_ENABLED` is present in a debug build. Regular `just dev` and release builds do not expose the bridge.

After the application opens, call `driver_session` with `action: "start"` and port `9223`. Confirm that the returned application identifier is `com.codingagent.va.desktop` before interacting with a reused session.

## Agent Workflow

1. Start the application with `just dev-mcp` and keep the process running.
2. Connect with `driver_session` and verify the application identifier.
3. Call `webview_dom_snapshot` before using screenshots. Use the structured DOM and accessibility state to locate controls.
4. Use `webview_find_element`, `webview_interact`, and `webview_keyboard` to operate the application.
5. Re-run `webview_dom_snapshot` after navigation or any material state change.
6. Use `read_logs` for frontend errors and `ipc_monitor` plus `ipc_get_captured` for Tauri command debugging.
7. Use `webview_execute_js` only when the normal DOM tools cannot expose the required state.
8. Use `ipc_execute_command` when directly exercising a Tauri command is necessary.
9. After editing frontend or Rust code, wait for the development application to restart and reconnect the driver session if needed.
10. Stop the session when the investigation is complete and run `just check`.

## Selector Rules

- Prefer semantic roles, accessible names, and stable `data-testid` attributes.
- Do not depend on generated CSS classes, DOM position, or local agent-session data.
- Add a meaningful `data-testid` when a critical control cannot be selected reliably through its accessible role and name.
- Verify every mutation by reading the resulting DOM state or logs instead of assuming the interaction succeeded.

## CLI Fallback

The companion CLI exposes the same tools when MCP tools are unavailable in the current agent session:

```bash
pnpm exec tauri-mcp driver-session start --port 9223
pnpm exec tauri-mcp webview-screenshot --file /tmp/coding-agent-va.png
pnpm exec tauri-mcp driver-session status
```

The CLI keeps the underlying MCP process alive between commands. Use the MCP tools directly when they are available.

## Security

- Treat the bridge as a privileged local debugging interface: it can execute JavaScript, operate the DOM, and invoke Tauri commands.
- Never register the bridge in release builds or enable it in the regular development command.
- Do not use the remote-host options or expose port `9223` outside a trusted development environment.
- Close the MCP-enabled application when debugging is complete.
