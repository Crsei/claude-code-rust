# UI Refactor Execution Guide

This document is for coding agents working on the Rust TUI refactor. Read this
before moving code. The goal is faster integration, faster agent orientation,
and smaller review surfaces without changing behavior.

## Current Contract

- Keep `crate::ui::*` public module paths stable unless a task explicitly says
  to change call sites.
- `mod.rs`, `tui.rs`, `app.rs`, `diff.rs`, `messages.rs`, and
  `permissions.rs` stay as root entry/facade files unless the current work
  package says otherwise.
- Prefer mechanical extraction over behavior edits.
- No new dependencies.
- Move tests and snapshots with the source file that owns them.
- Run targeted tests after each work package.

## Baseline Checks

Before starting a work package:

```powershell
git status --short
cargo fmt --all
cargo check -p allthecodes
```

Known non-UI failures may exist in the full package test suite. Do not treat
them as blockers for a UI-only refactor if targeted UI tests pass and the same
non-UI failures reproduce on the baseline.

Minimum verification after each package:

```powershell
cargo fmt --all
cargo check -p allthecodes
cargo clippy -p allthecodes --all-targets -- -D warnings
```

Run the package-specific tests listed below as well.

## Execution Order

Do the packages in this order. Do not combine packages unless explicitly asked.

1. `command_surface.rs`
2. `app.rs`
3. `tui.rs`
4. `command_palette.rs`
5. `permissions.rs`
6. `messages.rs`

Rationale: the first four files are the largest integration blockers. The last
two are high-fan-in facades and should be handled conservatively.

## Package 1: Split `components/command_surface.rs`

Problem:

- One file owns the command-surface enum, every concrete surface, domain data
  adapters, git diff conversion, MCP conversion, team/task conversion, memory
  options, and tests.

Target shape:

```text
components/command_surface/
  mod.rs
  surfaces/
    agents.rs
    config.rs
    diff.rs
    hooks.rs
    lsp_recommendation.rs
    mcp.rs
    memory.rs
    sandbox.rs
    skills.rs
    tasks.rs
    team.rs
  adapters/
    agents.rs
    diff.rs
    hooks.rs
    mcp.rs
    memory.rs
    tasks.rs
    team.rs
  tests.rs
```

Public API to preserve:

```rust
crate::ui::command_surface::{CommandSurface, CommandSurfaceOutcome}
```

Recommended steps:

1. Convert `components/command_surface.rs` into
   `components/command_surface/mod.rs`.
2. Move one surface at a time into `surfaces/<name>.rs`.
3. Move pure conversion/helper functions into `adapters/<domain>.rs`.
4. Keep `CommandSurface::for_slash_command`, `title`, `render`, and
   `handle_key` in `mod.rs`.
5. Move tests to `components/command_surface/tests.rs` or colocate small tests
   with their surface.

Targeted tests:

```powershell
cargo test -p allthecodes ui::command_surface
```

Stop conditions:

- Do not change slash-command behavior.
- Do not rewrite git/MCP/task/team data construction while extracting it.

## Package 2: Split `app.rs`

Problem:

- `App` owns state, input handling, keybinding dispatch, command-surface
  handling, mouse handling, workspace trust, voice, status-line payloads,
  transcript modes, rendering, and tests in one file.

Target shape:

```text
app.rs
app/
  input.rs
  render.rs
  status.rs
  transcript_mode.rs
  workspace_trust.rs
  voice.rs
  tests.rs
```

Public API to preserve:

```rust
crate::ui::app::{App, AppAction}
```

Recommended steps:

1. Move pure rendering helpers first:
   `render_workspace_trust_prompt`, `render_command_surface_overlay`,
   `render_suggestions`, `render_status_bar`, transcript/focus render helpers.
2. Move transcript key/mode logic next:
   `handle_transcript_key`, `scroll_transcript_*`, `commit_search`,
   `snap_to_current_match`.
3. Move voice methods and status-line payload helpers.
4. Move keybinding dispatch and prompt/history input handling last.
5. Keep `App` struct and constructor in `app.rs` until the end.

Targeted tests:

```powershell
cargo test -p allthecodes ui::app
```

Stop conditions:

- Do not change `AppAction` variants without updating `tui.rs` in the same
  package.
- Do not split fields out of `App` into nested state structs until simple
  method extraction is complete.

## Package 3: Split `tui.rs`

Problem:

- The terminal entry owns terminal setup, event loop, permission callback,
  engine spawning, SDK streaming reduction, command execution, subsystem event
  routing, export-to-editor, and tests.

Target shape:

```text
tui.rs
tui/
  terminal_guard.rs
  engine_events.rs
  commands.rs
  subsystem_events.rs
  export.rs
  tests.rs
```

Recommended steps:

1. Extract `TerminalGuard` and terminal setup/cleanup helpers.
2. Extract streaming state and SDK message handling.
3. Extract command execution helpers.
4. Extract subsystem event handlers.
5. Keep `run_tui` readable as the orchestration skeleton.

Targeted tests:

```powershell
cargo test -p allthecodes ui::tui
```

Stop conditions:

- Do not change async channel behavior while moving code.
- Do not change terminal raw-mode or mouse-capture cleanup semantics.

## Package 4: Split `components/command_palette.rs`

Problem:

- Palette state, filtering, rendering, command metadata, edit target discovery,
  path display, and snapshots live together.

Target shape:

```text
components/command_palette/
  mod.rs
  render.rs
  metadata.rs
  edit_targets.rs
  filter.rs
  tests.rs
  snapshots/
```

Public API to preserve:

```rust
crate::ui::command_palette::CommandPalette
```

Recommended steps:

1. Move `CommandMeta` and `command_meta` into `metadata.rs`.
2. Move `EditTarget` and edit target helpers into `edit_targets.rs`.
3. Move `filtered_commands`, `fuzzy_match`, and scoring into `filter.rs`.
4. Move buffer rendering helpers into `render.rs`.
5. Move snapshots with the test module.

Targeted tests:

```powershell
cargo test -p allthecodes ui::command_palette
```

Stop conditions:

- Snapshot diffs should be path-only after moving snapshots. Any text change
  needs a separate behavioral explanation.

## Package 5: Thin `permissions.rs`

Problem:

- Root `permissions.rs` is both a facade for many permission modules and the
  implementation file for `PermissionDialog`.

Target shape:

```text
permissions.rs
permissions/
  dialog_overlay.rs
```

Public API to preserve:

```rust
crate::ui::permissions::{PermissionChoice, PermissionDialog}
```

Recommended steps:

1. Move `PermissionChoice`, `PermissionDialog`, and dialog helper functions to
   `permissions/dialog_overlay.rs`.
2. Re-export them from root `permissions.rs`.
3. Keep child `pub mod` declarations in root `permissions.rs`.

Targeted tests:

```powershell
cargo test -p allthecodes ui::permissions
```

Stop conditions:

- Do not move the full permission feature tree in this package.

## Package 6: Thin `messages.rs`

Problem:

- Root `messages.rs` is both a facade for generated message modules and the
  implementation file for message rendering/wrapping.

Target shape:

```text
messages.rs
messages/
  render.rs
  wrap.rs
```

Public API to preserve:

```rust
crate::ui::messages::{render_messages, render_single_message}
```

Recommended steps:

1. Move wrapping helpers into `messages/wrap.rs`.
2. Move render dispatch and message-specific legacy renderers into
   `messages/render.rs`.
3. Re-export `render_messages` and `render_single_message` from root
   `messages.rs`.
4. Keep generated message module declarations in root `messages.rs`.

Targeted tests:

```powershell
cargo test -p allthecodes ui::messages
```

Stop conditions:

- Do not rewrite individual generated message components in this package.

## Review Checklist

Use this checklist before reporting completion:

- Public `crate::ui::*` imports still compile.
- Root files are smaller or more clearly facade-like.
- No behavior change was mixed into mechanical extraction.
- Tests and snapshots moved with their source owners.
- `cargo fmt --all` passed.
- `cargo check -p allthecodes` passed.
- `cargo clippy -p allthecodes --all-targets -- -D warnings` passed.
- Targeted UI tests for the package passed.
- If full `cargo test -p allthecodes` fails, the final report lists exact
  failures and states whether they are non-UI baseline failures.

## Commit Guidance

Prefer one commit per package. Use the repository Lore commit protocol. The
intent line should explain why the split happened, for example:

```text
Make command surfaces readable as independent UI domains
```

Record verification honestly in `Tested:` and `Not-tested:` trailers.
