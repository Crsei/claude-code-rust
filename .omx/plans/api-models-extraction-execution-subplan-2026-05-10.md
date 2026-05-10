# API and Models Extraction Execution Subplan

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Lane scope: `api` and model metadata only.
Execution defaults: `gpt-5.5`, `medium` reasoning, concise visible output.

## Scope

Extract model metadata and provider API transport out of
`crates/claude-code-rs/src` without changing provider behavior, streaming
semantics, auth resolution, token counting, or user-visible errors.

In scope:

- `model_registry.rs`, model aliases, provider capability metadata, pricing,
  and model mapping -> `crates/cc-models`.
- `api/**`, provider clients, streaming adapters, retry/timeout behavior,
  auth-bearing request construction, and token counting -> `crates/cc-api`.
- Temporary root compatibility shims only while imports are being migrated.

Out of scope:

- IPC protocol DTO ownership.
- Command UX changes.
- New providers or model behavior changes.
- New external dependencies.

## Ownership

| Responsibility | Owner | Notes |
| --- | --- | --- |
| Model aliases and canonical names | `cc-models` | No network/client deps |
| Provider capability flags and limits | `cc-models` | Keep UI/commands usable without HTTP clients |
| Pricing tables and fallback metadata | `cc-models` | Preserve current pricing behavior |
| Provider request/response payloads | `cc-api` | Only provider-specific DTOs |
| Streaming/SSE parsing | `cc-api` | Preserve malformed-event behavior |
| Retry, timeout, and transport errors | `cc-api` | Errors include provider/model/request context |
| Domain message DTOs | `cc-types` or protocol owner | Do not duplicate in `cc-api` |

Reject rule: no DTO may be copied into both `cc-models` and `cc-api`. If both
need it, move the shared type to the existing domain/protocol owner and import
it from there.

## Migration Slices

### Slice A: Inventory and move map

- Map all `crate::api` and `model_registry` imports.
- Classify each type as metadata, provider-specific transport, or shared domain
  DTO.
- Capture baseline checks:
  - `cargo test -p claude-code-rs api::`
  - `cargo test -p claude-code-rs commands::login`
  - `cargo test -p claude-code-rs engine::lifecycle`
  - `cargo metadata --no-deps`

Exit: a move table exists and no code movement has started.

### Slice B: Extract `cc-models`

- Move metadata-only files into `crates/cc-models/src`.
- Add exports for current callers.
- Rewire callers that only need metadata to `cc_models::*`.
- Verify `cc-models` does not depend on `reqwest`,
  `eventsource-stream`, provider auth, or HTTP transport crates.

Exit:

- `cargo test -p cc-models`
- `cargo tree -p cc-models` shows no network/client dependencies.

### Slice C: Extract `cc-api`

- Move provider clients, streaming, retry, token counting, and auth-bearing
  request code into `crates/cc-api/src`.
- Keep public function signatures stable where possible.
- Leave `crates/claude-code-rs/src/api/mod.rs` as a narrow `pub use cc_api::*`
  shim only if needed for a short transition.

Exit:

- `cargo test -p cc-api`
- Provider-specific offline tests still pass.
- Moved provider errors still expose provider, model id, request type, and
  status/context.

### Slice D: Rewire and delete shims

- Replace remaining root `crate::api::*` imports with `cc_api::*` or
  `cc_models::*`.
- Delete temporary root API shim once no call sites need it.
- Run dependency and file-size guards.

Exit:

- no `crate::api::` call sites outside the temporary shim.
- no moved production file exceeds the lane file-size limit without a split.

### Slice E: Final verification and tracking

- Run final lane gates.
- Save effects, defects, and follow-ups to the lane execution report.
- Update parent plan status only if the lane completes or a blocker changes the
  sequence.

## Review Gates

- `[review]` after Slice B: metadata/transport dependency boundary.
- `[review]` after Slice C: provider error visibility and behavior parity.
- `[review]` after Slice D: no root API shim dependency and no hidden reverse
  edges.
- `[final]` after Slice E: final report, known defects, and follow-up owners.

Each review must report `PASS`, `WARNING`, `BLOCKER`, or `ERROR`.

## File-Size and Refactor Guards

- Warn on touched Rust files over 500 LoC.
- Block batch commit on new or moved Rust files over 800 LoC unless the task
  explicitly splits the file in the same batch.
- If provider code grows while moving, split by provider and transport concern:
  `anthropic`, `openai`, `bedrock`, `vertex`, `stream`, `retry`, `errors`.

## Error Policy

- Do not add redundant validation layers in both `cc-models` and `cc-api`.
- Metadata incompatibility belongs in `cc-models`.
- Transport, HTTP, streaming, and provider payload errors belong in `cc-api`.
- Error messages must be explicit enough for a later agent to identify:
  provider, model id, request kind, HTTP/status context when available, and the
  failing parse/stream phase.

## Task Lines

Use `docs/scripts/workspace-crate-extraction-omx-tasks-2026-05-10.txt` for the
canonical next execution list. Lane-local tasks should follow this shape:

- `[checkpoint] api-models-00 - inventory current api/model imports, DTO ownership, dependency graph, and baseline tests.`
- `[build] api-models-01 - move model metadata into cc-models and rewire metadata-only callers.`
- `[review] api-models-02 - verify cc-models has no transport deps and model errors remain explicit.`
- `[build] api-models-03 - move provider transport, stream, retry, token-counting, and auth-bearing request code into cc-api.`
- `[review] api-models-04 - verify provider behavior, explicit diagnostics, and no duplicated DTO ownership.`
- `[final] api-models-05 - remove shims, run lane gates, and update execution report.`

## Tests

- Per batch:
  - `cargo fmt --all --check`
  - `cargo check --workspace --all-targets --message-format short`
- Slice B:
  - `cargo test -p cc-models`
  - `cargo tree -p cc-models`
- Slice C/D:
  - `cargo test -p cc-api`
  - `cargo test -p claude-code-rs api::`
  - `cargo test -p claude-code-rs commands::login`
  - `cargo test -p claude-code-rs engine::lifecycle`
- Final:
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`

## Tracking

Append batch outcomes to
`docs/archive/workspace-crate-extraction-execution-tracker-2026-05-10.md`:

- effects: files moved, APIs created, shims deleted.
- defects: failing tests, behavior diffs, provider error regressions.
- follow-ups: deferred splits, test gaps, dependency cleanup.

## Risks

- Hidden coupling between engine startup and root API helpers.
- Provider error behavior drifting while moving transport code.
- Metadata crate accidentally pulling HTTP dependencies.
- Shared DTOs duplicated instead of moved to their proper owner.
