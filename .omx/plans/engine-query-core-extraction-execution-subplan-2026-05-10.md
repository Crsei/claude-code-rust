# Engine and Query Core Extraction Execution Subplan

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Lane scope: `engine` and `query` runtime only.
Execution defaults: `gpt-5.5`, `medium` reasoning, concise visible output.

## Scope

Move `crates/claude-code-rs/src/query/**` into `crates/cc-query` and move
`crates/claude-code-rs/src/engine/**` into `crates/cc-engine`, preserving query
loop behavior, cancellation, lifecycle transitions, message submission, and
agent/runtime orchestration semantics.

Out of scope:

- Provider transport extraction.
- Tool handler ownership.
- Commands, IPC, daemon, UI, web, plugins, and teams migrations except for
  import rewiring required by this lane.

## Boundary Contract

| Concern | Owner | Rule |
| --- | --- | --- |
| Query loop execution | `cc-query` | Owns loop implementation and loop helpers |
| Message iteration/cancellation | `cc-query` | No UI/IPC/commands dependencies |
| Lifecycle transitions | `cc-engine` | Owns lifecycle state machine |
| Submit-message orchestration | `cc-engine` | Calls `cc-query` through explicit APIs |
| Agent runtime coordination | `cc-engine` | No binary-crate dependency |
| Shared types | `cc-types` / protocol owner | Do not define duplicate local copies |

Dependency direction: `cc-engine -> cc-query`; `cc-query` may depend on
low-level types/services/API crates, but must not depend on `cc-engine`,
`cc-commands`, `cc-ui`, or the binary crate.

## Migration Slices

### Slice A: Boundary prep

- Inventory all `crate::query` and `crate::engine` call sites.
- Add target crate exports that mirror current public entry points.
- Decide which types are loop-local and which belong in shared type crates.
- Capture baseline tests:
  - `cargo test -p claude-code-rs engine::lifecycle`
  - `cargo test -p claude-code-rs query::loop_helpers`
  - `cargo test -p claude-code-rs query::loop_impl`

### Slice B: Extract `query`

- Move `loop_impl`, `loop_helpers`, and loop-local modules to `cc-query`.
- Keep a temporary root `query` shim only while imports are migrated.
- Preserve loop ordering and cancellation behavior exactly.

Exit:

- `cargo test -p cc-query`
- old `claude-code-rs query::` targeted tests still pass through shim or are
  moved to the new crate.

### Slice C: Extract engine lifecycle and submit path

- Move lifecycle, dependency wiring, submit-message, and helpers to
  `cc-engine`.
- Replace inline query references with `cc_query` calls.
- Keep message ordering and state mutation timing unchanged.

Exit:

- `cargo test -p cc-engine`
- `cargo test -p claude-code-rs engine::lifecycle`

### Slice D: Extract agent runtime support

- Move `src/engine/agent/**` and engine-owned agent adapters.
- Do not move generic tool handlers in this lane.
- Keep agent/runtime errors explicit rather than hidden behind fallback defaults.

Exit:

- agent runtime tests pass.
- dependency graph has no reverse edge into `claude-code-rs`.

### Slice E: Shim deletion and graph cleanup

- Delete temporary root implementations.
- Keep only compatibility modules for one batch if required by downstream lanes.
- Remove stale imports and narrow Cargo dependencies.

Exit:

- no implementation-heavy `src/engine` or `src/query` modules remain.
- no crate depends on `claude-code-rs`.

## Review Gates

- `[review]` after Slice B: query loop parity and cancellation behavior.
- `[review]` after Slice C: lifecycle and submit-message parity.
- `[review]` after Slice D: dependency graph and agent/runtime error clarity.
- `[final]` after Slice E: shim deletion, file-size guard, and report update.

## File-Size and Refactor Guards

- Warn on touched Rust files over 500 LoC.
- Block on new or moved Rust files over 800 LoC unless split in the same batch.
- Split large engine files by actual ownership:
  - lifecycle state machine
  - submit-message orchestration
  - dependency assembly
  - agent runtime
  - test fixtures

## Error Policy

- Do not add defensive retry/fallback layers during extraction.
- Preserve existing error variants and messages unless crate boundaries require
  namespace-only changes.
- If a moved boundary adds context, include the phase, query id/session id when
  already available, and the failing engine/query operation.

## Task Lines

- `[checkpoint] engine-query-00 - inventory engine/query call sites, loop ownership, baseline tests, and dependency graph.`
- `[build] engine-query-01 - move query loop implementation and helpers into cc-query with temporary shim.`
- `[review] engine-query-02 - verify loop parity, cancellation behavior, and cc-query dependency boundary.`
- `[build] engine-query-03 - move lifecycle, submit_message, deps, and helpers into cc-engine.`
- `[review] engine-query-04 - verify lifecycle tests, submit path behavior, and no new engine monolith drift.`
- `[build] engine-query-05 - move agent runtime support owned by engine and rewire callers.`
- `[final] engine-query-06 - remove shims, run lane gates, and update execution tracker.`

## Tests

- Per batch:
  - `cargo fmt --all --check`
  - `cargo check --workspace --all-targets --message-format short`
  - `cargo metadata --no-deps`
- Lane tests:
  - `cargo test -p cc-query`
  - `cargo test -p cc-engine`
  - `cargo test -p claude-code-rs engine::lifecycle`
  - `cargo test -p claude-code-rs query::loop_helpers`
  - `cargo test -p claude-code-rs query::loop_impl`
- Final:
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`

## Tracking

Append outcomes to
`docs/archive/workspace-crate-extraction-execution-tracker-2026-05-10.md`.
Record effects, defects, follow-ups, dependency graph changes, and remaining
shim debt per batch.

## Risks

- `cc-engine` becoming the replacement monolith.
- Hidden dependency cycle between `cc-engine` and `cc-query`.
- Query loop cancellation/backpressure drift.
- Tests still tied to root module paths after moves.
