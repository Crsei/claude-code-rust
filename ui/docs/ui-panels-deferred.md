# Deferred operational panels

This file tracks upstream sample-tree panels that were reviewed during
Issue 05 (`Operational Panels For MCP, LSP, Team, And Shell`) but not
brought into the active frontend. Each entry names the upstream source,
the specific protocol gap that blocks the port, and a short note on
what it would take to unblock it later.

Keep this document honest: when a blocker is resolved — either by a
protocol addition or a deliberate scope decision — move the upgraded
panel into `ui/src/components/panels/` and remove its row from here.

## MCP

### `ui/examples/upstream-patterns/src/components/mcp/MCPListPanel.tsx`

Full editor panel with scope grouping (project / user / local /
enterprise) and per-server drill-down.

- **Blocker**: the Lite `McpServerStatusInfo` does not carry a
  `scope` — the upstream view groups servers under the file they were
  loaded from. `McpServerConfigEntry` (protocol.ts, added for the
  `/mcp` editor) *does* carry scope but is a different surface.
- **Unblock path**: merge `scope` into `McpServerStatusInfo` or wire
  the `/mcp` editor commands so the status panel can cross-reference
  config entries by name.

### `ui/examples/upstream-patterns/src/components/mcp/ElicitationDialog.tsx`

Interactive dialog driven by MCP server `elicitation/*` JSON-RPC events.

- **Blocker**: the Rust `mcp_event` protocol only emits
  `server_state_changed` today — there is no inbound elicitation event
  to dispatch on.
- **Unblock path**: add an `mcp_event::elicitation_request` shape in
  `src/mcp/` and forward it through the daemon.

### `ui/examples/upstream-patterns/src/components/mcp/MCPReconnect.tsx`

Per-server reconnect workflow with retry backoff visualization.

- **Blocker**: the current backend reconnects automatically without
  exposing retry counters or failure history over IPC.
- **Unblock path**: surface `reconnect_attempt` / `next_retry_at` on
  `server_state_changed` events.

## LSP

### `ui/examples/upstream-patterns/src/components/LspRecommendation/LspRecommendationMenu.tsx`

"Install suggested LSPs" flow reached from a menu hint.

- **Blocker**: `LspServerInfo` does not include a `recommended`
  bucket or an install action channel.
- **Unblock path**: teach the Rust LSP manager to publish a
  `recommended_languages` snapshot alongside the active list, and add
  an `lsp_install_request` IPC message.

### `ui/examples/upstream-patterns/src/components/DiagnosticsDisplay.tsx`

Inline diagnostics panel (file path, severity, source, message).

- **Blocker**: the `LspDiagnostic` shape exists in `protocol.ts` but
  the backend does not currently forward diagnostic batches to the
  frontend. `LspServerInfo` only reports aggregate server state.
- **Unblock path**: add an `lsp_event::diagnostics_updated` shape
  carrying `LspDiagnostic[]` keyed by file path.

## Teams

### `ui/examples/upstream-patterns/src/components/teams/TeamsDialog.tsx`

Dialog for browsing all teams + teammates with typing / selection
affordances.

- **Blocker**: routed through the upstream sample `AppState.teamContext`
  tree + `browser.ink` dialog primitives. We already have a static
  `TeamPanel`; a full dialog would need a keyboard-driven list widget
  that the Lite frontend doesn't share yet.
- **Unblock path**: either adopt a shared list widget (Issue 02 brought
  in `OrderedList` but not a selection list) or defer until Issue 06+
  adds a proper selector primitive.

## Shell

### `ui/examples/upstream-patterns/src/components/shell/ShellProgressMessage.tsx`

Live shell progress line while a `Bash` tool runs (bytes streamed,
elapsed).

- **Blocker**: the Rust `Bash` tool reports status through the
  generic `tool_activity` pipeline without emitting incremental
  progress frames (no byte counts, no heartbeat).
- **Unblock path**: add a tool-specific progress event (`bash_tick`?)
  that forwards accumulated stdout size + elapsed ms. Without those
  numbers there is nothing to animate.

### `ui/examples/upstream-patterns/src/components/shell/ExpandShellOutputContext.tsx`

"Expand truncated output" drawer for long Bash results.

- **Blocker**: the current `tool_result` message carries a single
  `output` string; the backend does not track a "full output" side
  channel to expand into.
- **Unblock path**: attach a content reference (path / handle) to
  truncated tool results so the frontend can request the full blob on
  demand.

## Agent progress

### `ui/examples/upstream-patterns/src/components/AgentProgressLine.tsx`

Single-line compacted agent progress under the input bar.

- **Blocker**: Lite already renders a richer `AgentTreePanel`.
  Bringing in the upstream collapsed view would duplicate that without
  the Lite tree's nested state.
- **Unblock path**: consider unifying under a shared agent view-model
  if we later want a collapsed status-bar view.
