# Commands Crate Extraction Execution Subplan

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Lane scope: `commands` -> `cc-commands`.
Execution defaults: `gpt-5.5`, `medium` reasoning, concise visible output.

## Scope

Move slash-command implementation from
`crates/claude-code-rs/src/commands/**` into `crates/cc-commands` after the
runtime/tool/API/IPС dependencies it needs are real crates. Preserve command
names, parsing, output shape, error text, e2e behavior, and dispatcher
semantics.

Out of scope:

- command UX redesign;
- splitting command categories for style only;
- UI command palette behavior changes except import rewiring;
- new dependencies.

## Ownership

| Concern | Owner | Notes |
| --- | --- | --- |
| command parser and registry | `cc-commands` | Single source of truth |
| `Command`, handler, result types | `cc-commands` / existing trait owner | Keep dispatcher trait stable |
| command implementations | `cc-commands` | No implementation-heavy root modules |
| root `commands` module | `claude-code-rs` | Temporary facade only |
| UI/headless/web callers | caller crates | Invoke through `cc-commands` boundary |

## Dispatcher Boundary

- `cc-commands` owns `get_all_commands`, `find_command`,
  `parse_command_input`, dispatcher construction, and handler registration.
- The root module may temporarily contain `pub use cc_commands::*`.
- Runtime layers invoke commands through the existing dispatcher trait or
  `cc_commands` helpers.
- Delete the root shim once no `crate::commands::` implementation imports
  remain.

## Migration Slices

### Slice A: Foundation and low-coupling commands

- Move parser, registry, dispatcher scaffolding, help/status/version/clear/exit,
  and other low-runtime-coupling commands.
- Update tests that assert command source layout.

### Slice B: Config, model, session, and memory commands

- Move config/profile/model/effort/features/session/export/memory commands.
- Preserve settings paths and explicit diagnostics.

### Slice C: Tool/service commands

- Move doctor, hooks, notify, IDE, LSP, Chrome, daemon, keybindings, voice,
  plugins, and MCP command clusters as their dependency crates permit.
- Do not pull UI/web/runtime internals into `cc-commands`.

### Slice D: Team, task, and agent commands

- Move team/task/coordinator/agent commands after `cc-teams` and `cc-tasks`
  boundaries are available.
- Keep command errors explicit: command name, target resource, and failure
  reason.

### Slice E: Call-site rewrite and shim cleanup

- Rewire `main`, `engine`, `ipc`, `daemon`, `web`, `ui`, and tests to
  `cc_commands`.
- Delete root command implementation modules.
- Narrow Cargo dependencies.

## Review Gates

- `[review]` after Slice A: dispatcher parity and parser behavior.
- `[review]` after Slice B: settings/session command errors and path behavior.
- `[review]` after Slice C: no high-level runtime imports into `cc-commands`.
- `[review]` after Slice D: team/task command integration.
- `[final]` after Slice E: shim deletion and e2e command smoke.

## File-Size and Refactor Guards

- Warn on touched Rust files over 500 LoC.
- Block on new or moved Rust files over 800 LoC unless split in the same batch.
- Known split-first candidates if moved as-is:
  - `config_cmd.rs`
  - `compact.rs`
  - `doctor.rs`
  - `memory.rs`
  - `team_cmd.rs`
  - `team_onboarding.rs`
  - `statusline_cmd.rs`

## Error Policy

- Do not add generic catch-all fallbacks that hide command failure source.
- Command errors should identify command name, subcommand/argument when
  relevant, target path/resource, and the owning subsystem that failed.
- Preserve existing user-facing text unless adding missing context is required
  for agent-debuggability.

## Task Lines

- `[checkpoint] commands-00 - inventory command modules, dispatcher call sites, source-layout tests, baseline command/e2e tests, and file-size hot spots.`
- `[build] commands-01 - move parser, registry, dispatcher, and low-coupling commands into cc-commands with root shim.`
- `[review] commands-02 - verify dispatcher parity, parser tests, and no output drift.`
- `[build] commands-03 - move config/model/session/memory command clusters and update tests.`
- `[review] commands-04 - verify explicit command errors and settings/session behavior.`
- `[build] commands-05 - move tool/service/MCP command clusters as dependency crates allow.`
- `[review] commands-06 - verify dependency graph and no UI/web/runtime imports in cc-commands.`
- `[build] commands-07 - move team/task/agent command clusters and rewire call sites.`
- `[final] commands-08 - remove command shims, run e2e command gates, and update execution tracker.`

## Tests

- `cargo test -p cc-commands`
- `cargo test -p claude-code-rs commands::`
- `cargo test -p claude-code-rs --test e2e_cli`
- `cargo test -p claude-code-rs --test e2e_plan_cmd`
- `cargo test -p claude-code-rs --test e2e_permissions` if permission commands move
- `cargo test -p claude-code-rs --test e2e_settings` if settings commands move
- final `cargo clippy --workspace --all-targets -- -D warnings`
- final `cargo test --workspace`

## Tracking

Append effects, defects, follow-ups, command output changes, file-size findings,
and remaining shim debt to
`docs/archive/workspace-crate-extraction-execution-tracker-2026-05-10.md`.

## Risks

- `cc-commands` pulling UI/web/runtime internals.
- E2E tests failing because they assert old source paths.
- Parser/dispatcher behavior drift during facade replacement.
- Command error visibility regressing into generic fallback text.
