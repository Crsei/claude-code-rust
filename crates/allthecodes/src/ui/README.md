# Rust TUI Layout

This directory contains the Rust terminal UI for `allthecodes`.

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

## Production Reachability

The Rust TUI does not wire every component directly to the production engine.
Production UI code is reachable through a small set of integration points:

- `tui.rs`: owns the terminal runtime loop. It receives terminal events, routes
  `AppAction`s, receives `EngineEvent`s, and calls `App::render`.
- `app.rs`: owns the live UI state. Fields on `App` are the main production
  component state list: prompt input, command palette, command surface,
  permission dialogs, question dialogs, notifications, agent navigation,
  transcript state, status line, and completions.
- `app/input.rs`: dispatches keyboard, mouse, and paste input into the active
  component. A component that is not reachable from this dispatcher generally
  cannot receive production input.
- `app/render.rs`: dispatches rendering for the active production surfaces. A
  component that is not reachable from this renderer generally cannot appear in
  the production TUI.
- `command_surface/mod.rs`: registers slash-command modal surfaces through
  `CommandSurface::for_slash_command`.
- `permissions/permission_request_router.rs`: routes structured tool permission
  requests into the production `PermissionDialog` rendering path.

Use this rule when deciding whether a UI module is production code:

```text
tui.rs
  -> App::handle_key_event / AppAction
  -> EngineEvent handling
  -> App::render
  -> CommandSurface::for_slash_command
  -> PermissionRequestRouter
```

If a module cannot be reached from one of those paths, it is test-only or
snapshot-only until it is explicitly integrated.

### AppAction vs EngineEvent

`AppAction` is the UI-to-main-loop boundary:

```text
component input -> App::handle_key_event -> AppAction -> tui.rs
```

Examples: prompt submit, abort, quit, permission response, question response,
agent thread selection, LSP recommendation response, transcript export, and
copy-message actions.

`EngineEvent` is the engine-to-main-loop boundary:

```text
QueryEngine / tools / hooks -> EngineEvent -> tui.rs -> App state
```

`EngineEvent` is not a component registration or initialization mechanism. It
only carries engine-side notifications into the TUI:

- `Sdk`: streaming/query SDK messages from `engine.submit_message`.
- `ToolProgress`: progress updates from long-running tools.
- `PermissionRequest`: tool permission request, displayed via
  `App::show_permission_request`.
- `QuestionRequest`: `AskUserQuestion` request, displayed via
  `App::show_question_dialog`.
- `HookPermissionDecision` and `PermissionDecisionDebug`: permission diagnostics
  rendered as notifications.
- `Done`: query stream completion.

Component initialization normally happens through `App::new` and `run_tui`
seeding state from `engine.app_state()`. Component changes normally update
`App` directly or return an `AppAction`; they do not emit `EngineEvent`.

### Keeping Test-Only Helpers Gated

Some renderer/helper modules intentionally remain behind `#[cfg(test)]` even
though they are related to production feature areas. The reason is reachability,
not feature priority. Until they are connected to `App`, `CommandSurface`,
`PermissionRequestRouter`, or the TUI main loop, enabling them in production
would reintroduce unused-code warnings.

Keep these groups test-only until they have a real production entry point:

- Permission helper renderers such as `permission_dialog`,
  `permission_prompt`, `permission_request`, `permission_request_title`,
  `permission_explanation`, `permission_rule_explanation`, and
  `use_shell_permission_feedback`. The production `PermissionDialog` is already
  connected to the structured router, but these helpers are still only consumed
  by snapshots.
- `file_permission_dialog` helpers such as `file_permission_dialog`,
  `ide_diff_config`, `use_file_permission_dialog`, and
  `use_permission_handler`. `permission_options` is used by the production
  file-tool router, but the full file dialog state, IDE diff state, and decision
  helpers still lack a real TUI entry point.
- Permission rule editor renderers such as `add_permission_rules`,
  `add_workspace_directory`, `permission_rule_input`, `recent_denials_tab`,
  `remove_workspace_directory`, `workspace_tab`, plus `WorkspaceDirectory` and
  `RecentDenial`. The current `/permissions` command surface still uses form
  tabs and slash-command fill/submit actions instead of these renderers.

## Responsibility Folders

### `components/`

Interactive widgets and modal surfaces composed by `app.rs`.

Examples: bottom pane, overlays, welcome view, status widget. Command palette,
command surface, prompt input, and selection surface source lives in this root
UI module tree and is exposed through the `crate::ui::*` facade.

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
