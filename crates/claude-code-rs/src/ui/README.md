# Rust TUI Layout

This directory contains the Rust terminal UI for `claude-code-rs`.

For the step-by-step large-file refactor sequence, see
[`REFACTOR_EXECUTION.md`](REFACTOR_EXECUTION.md).

Root files are reserved for entry points and module facades:

- `mod.rs`: public module registry. It keeps stable `crate::ui::*` paths and points most modules at responsibility folders with `#[path]`.
- `tui.rs`: terminal runtime loop. It owns raw terminal setup, event polling, engine/subsystem channels, and calls into `App`.
- `app.rs`: main UI state and integration point. It owns prompt state, transcript state, command surfaces, permission dialogs, and rendering orchestration.
- `diff.rs`, `messages.rs`, `permissions.rs`: facade modules for the same-named feature folders. They stay at root so their child modules keep simple Rust module paths.

## Dependency Direction

The intended dependency flow is:

```text
tui.rs -> app.rs -> components / feature domains -> rendering + input helpers
                         |
                         -> runtime / platform adapters
```

Leaf modules should not drive the terminal loop directly. New UI code should prefer adding a small feature-domain module or a component and have `app.rs` or `tui.rs` integrate it explicitly.

## Responsibility Folders

### `components/`

Interactive widgets and modal surfaces composed by `app.rs`.

Examples: command palette, command surface, prompt input, selection surface, bottom pane, overlays, welcome view, status widget.

### `input/`

Keyboard/editing helpers and parser-like UI input logic.

Examples: clipboard helpers, Vim state, slash-command parsing, mention encoding, file search, tabbed form navigation.

### `rendering/`

Shared rendering primitives and presentation helpers.

Examples: theme, markdown rendering, spinner/shimmer, virtual scrolling, history/tool activity rendering, git diff helper.

### `runtime/`

Non-visual UI state machines, event routing, transcript/session helpers, and UI verification fixtures.

Examples: event router, frame requester, streaming controller, transcript state, session log, visual regression surfaces.

### `platform/`

Adapters for terminal, browser, audio, debug, and environment behavior.

Examples: terminal environment/integration, browser adapter, custom terminal, audio device detection.

### `helpers/`

Small cross-feature helpers that are not themselves UI components.

### `status/`

Status-line glue that bridges the UI to `cc-engine` status-line support.

## Feature Domains

These folders own larger user-facing areas and usually expose their own `mod.rs`:

- `agents/`
- `diff/`
- `hooks/`
- `lsp_recommendation/`
- `mcp/`
- `memory/`
- `messages/`
- `notifications/`
- `permissions/`
- `skills/`
- `tasks/`
- `teams/`

Feature domains may depend on shared `components`, `input`, and `rendering` helpers. Shared helpers should not depend back on feature domains unless the coupling is intentionally local and documented.

## Snapshot Review Export

Use the workspace script to regenerate accepted UI snapshots and collect them
into one flat review folder:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\export-ui-snapshots.ps1
```

Default output is `target/ui-snapshots/`. Each file is named
`<module>__NN__<snapshot>.txt`, and `index.md` lists the source module for quick
navigation. Pass `-CheckOnly` to verify existing snapshots without accepting
new render output, or `-SkipTests` to only re-export already accepted `.snap`
files.
