# IPC, Protocol, Client, and Runtime Extraction Execution Subplan

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Lane scope: IPC wire types, client transport, and IPC runtime integration.
Execution defaults: `gpt-5.5`, `medium` reasoning, concise visible output.

## Scope

Move IPC protocol contracts, client transport, and headless/app-server runtime
integration out of `crates/claude-code-rs/src/ipc/**`, while preserving JSON
wire compatibility, event ordering requirements, backpressure behavior, and
diagnostic visibility.

In scope:

- `src/ipc/protocol/**` -> `cc-ipc-protocol`.
- JSONL/JSON-RPC client transport, callbacks, sink/ingress, and query-runner
  client paths -> `cc-ipc-client`.
- Runtime orchestration handlers -> `cc-ipc`.
- IPC-facing `tools/send_message.rs`, `tools/team_spawn.rs`,
  `tools/system_status.rs` handoff to their natural owning crates.

Out of scope:

- UI extraction.
- Model/API provider behavior.
- Broad command migration except call-site rewiring needed by IPC.

## Ownership

| Concern | Owner | Notes |
| --- | --- | --- |
| Wire DTOs and serde invariants | `cc-ipc-protocol` | No runtime deps |
| Roundtrip fixtures | `cc-ipc-protocol` | Pin field/tag compatibility |
| Client transport and bounded queues | `cc-ipc-client` | Owns lossless/best-effort classification |
| Headless/app-server runtime | `cc-ipc` | Depends on protocol/client and runtime crates |
| Team send/spawn behaviors | `cc-teams` | IPC adapters may live in `cc-ipc` |
| System status tool | `cc-ipc` or daemon owner | Choose by runtime dependency direction |
| Shared non-wire types | `cc-types` | Do not bury public wire DTOs here |

## Lossless and Best-Effort Events

Lossless events must be delivered or fail with an explicit diagnostic:

- ready/start/end lifecycle messages.
- transcript deltas, assistant messages, tool use, tool result.
- permission/question requests and responses.
- plan workflow events and conversation replacement.
- non-recoverable errors and command-result events that drive UI actions.

Best-effort events may drop under pressure but never silently:

- progress ticks.
- usage/status snapshots.
- notification/subsystem snapshots where latest state is sufficient.

Best-effort replacement policy:

- Drop oldest replaceable status event.
- Increment structured counters: `drop_count`, `queue_depth`,
  `last_dropped_type`.
- Surface lag diagnostics through an explicit error/diagnostic event.

## Migration Slices

### Slice A: Baseline and protocol inventory

- Count protocol DTOs, serde fixtures, and current IPC tests.
- Capture line-size baseline for `src/ipc`, related tool files, and daemon IPC
  bridge files.
- Run baseline tests:
  - `cargo test -p claude-code-rs ipc::`
  - `cargo test -p claude-code-rs tools::send_message`
  - `cargo test -p claude-code-rs tools::team_spawn`
  - `cargo test -p claude-code-rs tools::system_status`

### Slice B: Extract `cc-ipc-protocol`

- Move protocol modules and roundtrip tests.
- Replace imports with `cc_ipc_protocol::*`.
- Keep root protocol shim only for short transition.

Exit:

- `cargo test -p cc-ipc-protocol`
- no dependency from protocol crate to engine, daemon, UI, or binary.

### Slice C: Extract `cc-ipc-client`

- Move transport codec, sink, ingress, callbacks, query-runner client path, and
  bounded queue logic.
- Add explicit event class tests for lossless and best-effort handling.
- Ensure parse failures and queue pressure emit debuggable diagnostics.

Exit:

- `cargo test -p cc-ipc-client`
- lossless events are never silently dropped.

### Slice D: Extract `cc-ipc` runtime

- Move runtime handlers and subsystem/agent IPC orchestration.
- Preserve initialization, callback registration, and event fanout semantics.
- Keep facade traits narrow to avoid binary crate cycles.

Exit:

- `cargo test -p cc-ipc`
- `cargo test -p claude-code-rs ipc::`

### Slice E: Domain handoff

- Move or rewire `send_message`, `team_spawn`, and `system_status` to their
  owning crates.
- Move daemon-facing IPC handlers to `cc-daemon` only where dependency
  direction stays clean.

Exit:

- `cargo test -p cc-teams`
- `cargo test -p cc-daemon`
- touched `claude-code-rs tools::` tests pass.

### Slice F: Cleanup and final verification

- Delete root IPC shims after imports are migrated.
- Run final workspace gates and update tracker.

## Review Gates

- `[review]` after Slice B: protocol drift and runtime dependency review.
- `[review]` after Slice C: queue/drop policy and error visibility.
- `[review]` after Slice D: runtime dependency graph and duplicated safety
  checks.
- `[review]` before final cleanup: file-size split review.
- `[final]` after Slice F: report and follow-up closure.

## File-Size and Refactor Guards

- Warn on touched Rust files over 500 LoC.
- Block on new or moved Rust files over 800 LoC unless split in the same batch.
- Split known large IPC areas before extending them:
  - agent settings
  - ingress
  - SDK mapper
  - subsystem events/types/handlers
  - daemon route/process state bridges

## Error Policy

- Do not layer identical validation checks across protocol, client, and runtime.
- Protocol validates serialization shape.
- Client validates transport parsing, event class, and queue behavior.
- Runtime validates lifecycle context and subsystem routing.
- Every pressure/drop/parse failure must include event type, queue depth where
  applicable, and source phase.

## Task Lines

- `[checkpoint] ipc-00 - record IPC wire inventory, line-size baseline, metadata graph, and baseline ipc/tool tests.`
- `[build] ipc-01 - move protocol DTOs and serde roundtrip tests into cc-ipc-protocol.`
- `[review] ipc-02 - verify no wire drift and no runtime dependencies in cc-ipc-protocol.`
- `[build] ipc-03 - move client transport, sink, ingress, callbacks, and query-runner client path into cc-ipc-client.`
- `[review] ipc-04 - verify lossless/best-effort queue tests and explicit pressure diagnostics.`
- `[build] ipc-05 - move runtime IPC orchestration handlers into cc-ipc and rewire call sites.`
- `[review] ipc-06 - verify dependency graph, behavior parity, and no duplicated safety layers.`
- `[build] ipc-07 - hand off team/send/status IPC-facing tools to owning runtime crates.`
- `[final] ipc-08 - remove shims, run final gates, and update execution tracker.`

## Tests

- `cargo test -p cc-ipc-protocol`
- `cargo test -p cc-ipc-client`
- `cargo test -p cc-ipc`
- `cargo test -p cc-teams`
- `cargo test -p cc-daemon`
- `cargo test -p claude-code-rs ipc::`
- `cargo test -p claude-code-rs tools::send_message`
- `cargo test -p claude-code-rs tools::team_spawn`
- `cargo test -p claude-code-rs tools::system_status`
- final `cargo clippy --workspace --all-targets -- -D warnings`
- final `cargo test --workspace`

## Tracking

Record implemented effects, defects, follow-ups, lossless/best-effort proofs,
line-size guard output, and dependency graph findings in
`docs/archive/workspace-crate-extraction-execution-tracker-2026-05-10.md`.

## Risks

- Event queue policy hiding behavior regressions.
- `cc-ipc` accidentally depending on binary internals.
- Protocol enum/field drift.
- Domain handoff expanding into commands/daemon work beyond this lane.
