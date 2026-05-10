# Tools and Tasks Extraction Execution Subplan

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Lane scope: `tools`, `cc-tools`, and `cc-tasks` only.
Execution defaults: `gpt-5.5`, `medium` reasoning, concise visible output.

## Scope

Extract task-domain code into `crates/cc-tasks` and keep `crates/cc-tools`
focused on shared tool definitions, registry/spec helpers, and thin adapters.
This lane must not turn `cc-tools` into a runtime monolith.

In scope:

- task store/state/output/task handlers -> `cc-tasks`;
- shared tool specs, schemas, registry/selection helpers, and output formatting
  -> `cc-tools`;
- tool tests moved toward the owning crate;
- temporary root shims while call sites migrate.

Out of scope:

- UI behavior changes;
- provider/API changes;
- IPC protocol changes beyond tool adapter imports;
- command UX changes except test rewiring needed for task/tool calls.

## File Ownership

| Current area | Target owner | Notes |
| --- | --- | --- |
| `src/tools/tasks.rs` and task store/output modules | `cc-tasks` | Core task domain behavior |
| task parsing/validation/results | `cc-tasks` | Structured errors, no silent fallback |
| task-related unit tests | `cc-tasks` | Move deterministic tests next to owner |
| generic tool specs and schemas | `cc-tools` | No engine/runtime dependency unless handler-owned |
| registry/selection helpers | `cc-tools` | Keep pure where possible |
| cross-tool integration tests | `cc-tools` or `claude-code-rs` | Only where integration is real |
| `send_message.rs`, `team_spawn.rs` | `cc-teams` lane | Do not bury team runtime in `cc-tools` |
| `system_status.rs` | `cc-ipc`/daemon lane | Natural runtime owner decides |
| LSP tool adapter | `cc-lsp-service` lane | Keep LSP ownership separate |

## Migration Slices

### Slice A: Boundary lock

- Inventory all `crate::tools` task-related call sites.
- Create/prepare `cc-tasks` exports without moving behavior.
- Decide which `Tool` trait/type imports remain in `cc-engine::types` or move
  through `cc-types`.
- Capture baseline:
  - `cargo test -p claude-code-rs tools::`
  - `cargo test -p claude-code-rs commands::tasks`
  - `cargo metadata --no-deps`

### Slice B: Move task data and state

- Move task domain types, store/state/output retention, and task result data.
- Keep visibility explicit (`pub` only at crate boundary, `pub(crate)` inside).
- Preserve existing persistence paths and JSON shapes.

Exit:

- `cargo check -p cc-tasks`
- `cargo test -p cc-tasks`

### Slice C: Move task execution and handlers

- Move task parsing, validation, dispatch, and tool handlers.
- Keep one canonical validation path; do not add duplicate defensive wrappers in
  both adapter and domain layers.
- Errors should include task id/name where available, action, workspace/path
  context, and failure reason.

Exit:

- task behavior tests pass in `cc-tasks`;
- invalid task payload, missing worktree, and unsupported action tests produce
  explicit errors.

### Slice D: Slim `cc-tools`

- Move shared specs/registry helpers to `cc-tools`.
- Keep task adapter in `cc-tools` as a thin call into `cc-tasks`.
- Do not pull engine/query/UI/IPC runtime into `cc-tools` for convenience.

Exit:

- `cargo check -p cc-tools`
- no new dependency cycle from `cc-tools` to high-level runtime crates.

### Slice E: Domain handoff and shim cleanup

- Leave team/system/LSP tool files to their natural lanes unless this lane must
  add temporary imports.
- Delete root task/tool implementation shims once call sites are migrated.
- Update report and parent plan status only for completed moves.

## Review Gates

- `[review]` after Slice A: ownership map and dependency boundary.
- `[review]` after Slice C: task behavior parity and explicit error handling.
- `[review]` after Slice D: `cc-tools` remains registry/spec focused.
- `[final]` after Slice E: root shim cleanup, report update, and final lane
  gates.

## File-Size and Refactor Guards

- Warn on touched Rust files over 500 LoC.
- Block on new or moved Rust files over 800 LoC unless split in the same batch.
- Split task code by concern before extending:
  - `parse`
  - `dispatch`
  - `errors`
  - `state`
  - `store`
  - `output`
  - `tests`

## Error Policy

- No redundant safety layers: validation happens once at the domain boundary,
  with adapters adding only caller context.
- Do not hide failures behind default task states.
- Error messages must identify action, task id/name when available, path or
  workspace when relevant, and the failed operation.

## Task Lines

- `[checkpoint] tools-tasks-00 - inventory tool/task call sites, task persistence shapes, baseline tests, dependency graph, and file-size hot spots.`
- `[build] tools-tasks-01 - scaffold cc-tasks boundary and add task-domain exports without behavior changes.`
- `[review] tools-tasks-02 - verify boundary ownership and no cc-tools runtime-monolith drift.`
- `[build] tools-tasks-03 - move task state/store/output/result types into cc-tasks and migrate tests.`
- `[build] tools-tasks-04 - move task parsing, validation, dispatch, and handlers into cc-tasks.`
- `[review] tools-tasks-05 - verify task parity, explicit error diagnostics, and file-size guard output.`
- `[build] tools-tasks-06 - slim cc-tools to specs/registry/thin adapters and rewire call sites.`
- `[final] tools-tasks-07 - delete task/tool shims, run lane gates, and update execution tracker.`

## Tests

- `cargo check -p cc-tasks`
- `cargo test -p cc-tasks`
- `cargo check -p cc-tools`
- `cargo test -p cc-tools`
- `cargo test -p claude-code-rs tools::`
- `cargo test -p claude-code-rs commands::tasks`
- final `cargo clippy --workspace --all-targets -- -D warnings`
- final `cargo test --workspace`

## Tracking

Append effects, defects, follow-ups, file-size findings, and dependency graph
notes to
`docs/archive/workspace-crate-extraction-execution-tracker-2026-05-10.md`.

## Risks

- `cc-tools` becoming a new monolith.
- Task persistence or JSON shape drift.
- Hidden coupling between task handlers and engine/query internals.
- Tests split incorrectly between unit ownership and integration coverage.
