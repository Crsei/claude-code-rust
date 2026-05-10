# CC UI Late Extraction Execution Subplan

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Lane scope: late `crates/cc-ui` extraction only.
Execution defaults: `gpt-5.5`, `medium` reasoning, concise visible output.

## Scope

Extract Rust TUI/UI code from `crates/claude-code-rs/src/ui/**` into
`crates/cc-ui` after the protocol/client/runtime boundaries are stable. This is
a late lane and should not start until IPC protocol/client extraction is green.

Out of scope:

- Starting UI extraction before IPC protocol/client stability.
- Non-UI behavior changes.
- Reworking visual design or command semantics during the move.

## Prerequisites

- `cc-ipc-protocol` and `cc-ipc-client` are stable enough for UI imports.
- Current project instruction still scopes active UI source to
  `crates/claude-code-rs/src/ui/`; if that instruction is still active, this
  lane begins with a boundary review before moving files.
- Snapshot and PTY baselines are captured.
- Dirty worktree baseline is recorded and excluded from runner commits.

## Ownership

| Concern | Owner | Notes |
| --- | --- | --- |
| UI crate scaffold | `cc-ui` | New crate, no dependency back to binary |
| Root UI facade | `claude-code-rs/src/ui/mod.rs` | Temporary `pub use cc_ui::*` only |
| App/tui entry files | `cc-ui` | Move after facade compiles |
| Components and surfaces | `cc-ui` | Move tests/snapshots with components |
| Runtime/render/input/platform | `cc-ui` | Preserve explicit diagnostics |
| Feature domains | `cc-ui` | Agents, tasks, teams, hooks, memory, skills, etc. |
| Reports/status docs | docs owner | Archive effects, defects, follow-ups |

## Migration Slices

### Slice A: Baseline and boundary review

- Read UI refactor guide if present.
- Capture current snapshot and PTY baseline.
- Map UI imports that depend on root runtime modules.
- Confirm IPC/client boundaries are available.

Exit:

- `cargo test -p claude-code-rs ui::`
- `cargo test -p claude-code-rs --test e2e_terminal`
- `cargo test -p claude-code-rs --test e2e_pty`

### Slice B: Scaffold `cc-ui` and facade

- Add `crates/cc-ui`.
- Keep root UI facade stable.
- Do not move behavior in the first scaffold commit unless required to compile.

Exit:

- `cargo check -p cc-ui`
- `cargo check -p claude-code-rs --message-format short`

### Slice C: Entry and app shell

- Move `app.rs`, `tui.rs`, `messages.rs`, permissions/diff entry points, and
  immediate tests.
- Keep `crate::ui::*` compatibility until downstream imports migrate.

### Slice D: Components and command surfaces

- Move command palette/surface, selection surfaces, input widgets, tabs,
  status icons, prompt input, bottom pane, history/search components, tests, and
  snapshots.

### Slice E: Runtime, rendering, input, and platform

- Move runtime, rendering, platform, persistent history/session log, status, and
  visual regression modules.
- Preserve runtime/render errors as explicit diagnostics.

### Slice F: Feature domains

- Move agents, tasks, teams, MCP, permissions, memory, hooks, LSP
  recommendation, skills, notifications, and diff domains.

### Slice G: Shim cleanup and final verification

- Delete root UI implementation shims.
- Re-run snapshot/PTY/e2e gates.
- Update execution tracker and status docs.

## Review Gates

- `[checkpoint]` before any move: baseline and boundary readiness.
- `[review]` after Slice C: facade stability and entry path behavior.
- `[review]` after Slice D: snapshot diff intent and component file-size guard.
- `[review]` after Slice E: runtime/render error visibility and PTY tests.
- `[review]` after Slice F: feature-domain consistency and no over-generalized
  abstractions.
- `[final]` after Slice G: reports and follow-up ledger.

## File-Size and Refactor Guards

- Warn on touched Rust files over 500 LoC.
- Block new or moved Rust files over 800 LoC unless split in the same batch.
- For UI batches, split when more than 8 files change unless the task is an
  explicit mechanical move with tests/snapshots.
- Do not bury multiple UI domains into a single catch-all module.

## Error Policy

- Remove redundant wrappers that silently convert render/runtime failures into
  defaults.
- Preserve fail-fast behavior for missing data.
- UI diagnostics should include domain, render/runtime phase, path/session id
  when already available, and the visible surface affected.

## Task Lines

- `[checkpoint] ui-00 - verify IPC/client prerequisites, capture UI snapshot/PTY baseline, and record UI import map.`
- `[build] ui-01 - scaffold cc-ui and root facade without behavior changes.`
- `[build] ui-02 - move app/tui/messages entry files and immediate tests into cc-ui.`
- `[review] ui-03 - verify facade stability, entry tests, and no behavior drift.`
- `[build] ui-04 - move command surfaces, palette, selection/input widgets, tests, and snapshots.`
- `[review] ui-05 - verify snapshot intent, file-size guard, and no redundant fallback wrappers.`
- `[build] ui-06 - move runtime, rendering, input, platform, status, and persistent history modules.`
- `[review] ui-07 - run PTY/snapshot subset and validate explicit runtime diagnostics.`
- `[build] ui-08 - move feature-domain UI modules in ownership groups.`
- `[final] ui-09 - remove shims, run final gates, and update status/report docs.`

## Tests

- `cargo test -p cc-ui`
- `cargo test -p claude-code-rs ui::`
- `cargo test -p claude-code-rs --test e2e_terminal`
- `cargo test -p claude-code-rs --test e2e_pty`
- `cargo test -p claude-code-rs --test e2e_tools`
- `cargo test -p claude-code-rs --test e2e_permissions` if permissions UI moves
- snapshot export/check command if available:
  `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/export-ui-snapshots.ps1 -CheckOnly`

## Tracking

Append effects, defects, snapshot changes, PTY status, file-size findings, and
follow-ups to
`docs/archive/workspace-crate-extraction-execution-tracker-2026-05-10.md`.
Update `docs/WORK_STATUS.md`, `docs/IMPLEMENTATION_GAPS.md`, or
`docs/KNOWN_ISSUES.md` only when the lane actually crosses those documented
boundaries.

## Risks

- `cc-ui` pulling binary internals and preserving hidden coupling.
- Snapshot churn obscuring behavior drift.
- PTY/runtime event regressions.
- Large mechanical moves bypassing module split review.
