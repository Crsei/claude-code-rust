# Workspace Crate Extraction Execution Tracker

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Runner: `scripts/run-workspace-crate-extraction-omx.ps1`

This file is the durable execution ledger for the workspace crate extraction
lanes. Execution agents must append to it after green batches, review gates, and
final lane closeout.

## Required Batch Entry Template

```markdown
## Batch NN - <lane/task label>

Status: PASS | WARNING | BLOCKER | ERROR
Commit: <hash or not committed>
Tasks:
- <task line>

Implemented effects:
- <files moved, APIs added, shims removed>

Defects and divergences:
- <test failure, behavior drift, diagnostic issue, or none>

Follow-ups:
- <owner, exact next action, risk if deferred>

Verification:
- <command>: <pass/fail/blocked>

File-size/refactor findings:
- <path, line count, action taken>

Dependency graph notes:
- <new edges, removed edges, cycle checks>

Error visibility notes:
- <explicit errors added or preserved, redundant defensive layers removed>
```

## Lane Closeout Requirements

For each lane, add:

- implemented effects;
- defects and known divergences;
- follow-up items with owners;
- verification evidence;
- line-size/refactor guard result;
- dependency graph result;
- remaining risks and whether they block the next lane.

## Current Status

No extraction execution has started from this tracker. The current commit adds
the planning and runner control surface only.

## Batch 00 - api-models-00 checkpoint

Status: PASS
Commit: not committed
Tasks:
- [checkpoint] api-models-00 - Inventory current api/model imports, DTO ownership,
  dependency graph, and baseline tests for api::, commands::login, and
  engine::lifecycle. No code moved.

Implemented effects:
- Documentation-only inventory added to this tracker.
- No Rust code, DTO ownership, module declarations, or imports were changed.

DTO ownership:
- `api::client` owns the current cross-provider request/client DTOs:
  `ApiProvider`, `MessagesRequest`, `ExactTokenCount`, `ApiClientConfig`, and
  `ApiClient`.
- `api::providers` owns provider catalog and capability DTOs:
  `ProviderProtocol`, `StreamingSupport`, `ProviderSupportStatus`,
  `ProviderCapabilities`, and `ProviderInfo`.
- Provider adapters own provider-local wire/auth DTOs:
  `bedrock::{BedrockAuth, BedrockStreamProvider}`,
  `vertex::{VertexAccessToken, VertexStreamProvider}`,
  `google_provider::{GeminiResponse, GeminiCandidate, GeminiContent,
  GeminiPart, GeminiFunctionCall, GeminiUsage}`, and
  `sigv4::{AwsCredentials, SignRequest, SignedHeaders}`.
- Shared stream/result DTOs are still owned by `types::message`; `api::*`
  imports `StreamEvent`, `ContentBlock`, `MessageDelta`, `AssistantMessage`,
  and `Usage` instead of owning canonical message shapes.
- `engine::lifecycle` owns lifecycle state DTOs:
  `QueryEngine`, `QueryEngineState`, `QueryEngineDeps`, `UsageTracking`,
  `PermissionDenial`, `AbortReason`, plus local submit-turn helper structs.
- `commands::login` owns only command handlers and pending OAuth command state:
  `LoginHandler`, `login_code::LoginCodeHandler`, and private
  `login_code::PendingOAuth`.

Dependency graph notes:
- `api::mod` exports `bedrock`, `client`, `google_provider`, `model_mapping`,
  `openai_compat`, `pricing`, `providers`, `retry`, `sigv4`,
  `stream_provider`, `streaming`, and `vertex`.
- `api::client` depends inward on `api::{providers, retry, streaming,
  stream_provider, bedrock, vertex, model_mapping}` and on
  `types::message`; it is the current construction and dispatch hub.
- Provider stream modules depend on `api::client::MessagesRequest` and emit
  `types::message::StreamEvent`; `stream_provider` dispatches to
  `openai_compat`, `google_provider`, `bedrock`, and `vertex` implementations.
- `commands::login` depends on `auth`, `auth::oauth`, and directly reads
  `api::client`, `api::bedrock`, and `api::vertex` helpers for cloud session
  status and enablement.
- `engine::lifecycle::{helpers,deps,submit_message}` depends directly on
  `api::client::{ApiClient, MessagesRequest}` and `api::streaming`; lifecycle
  also depends on `commands`, `query`, `compact`, `session`, `tools`, and
  `types::*`.
- No new dependency cycles were introduced because this checkpoint did not
  change code.

Defects and divergences:
- `cargo test -p claude-code-rs commands::login` also runs
  `commands::login_code` tests because the filter matches the login-code module.
- `api::client::MessagesRequest` is consumed by lifecycle and provider adapters,
  so later crate extraction must either keep it in the API boundary or introduce
  an explicit shared request DTO. This checkpoint intentionally did not choose
  or implement that split.
- Several guard-sized files already exceed 700 lines and should be split before
  growing: `engine/lifecycle/deps.rs` 2221 lines,
  `engine/lifecycle/submit_message.rs` 1283 lines, `api/client/tests.rs` 1034
  lines, `api/client/mod.rs` 987 lines, `api/google_provider.rs` 981 lines,
  `api/openai_compat.rs` 869 lines, `api/vertex.rs` 803 lines,
  `api/bedrock.rs` 719 lines, and `api/streaming.rs` 704 lines.

Follow-ups:
- Next API/model extraction task should decide the target home for
  `MessagesRequest` before editing lifecycle or provider adapters.
- If login cloud status remains in `commands::login`, preserve the current
  explicit dependency on `api::{client,bedrock,vertex}` or add a narrow facade
  with tests before changing it.
- Split touched oversized Rust files before adding new behavior to them.

Verification:
- `cargo test -p claude-code-rs api::`: pass, 144 passed.
- `cargo test -p claude-code-rs commands::login`: pass, 13 passed.
- `cargo test -p claude-code-rs engine::lifecycle`: pass, 49 passed.

File-size/refactor findings:
- No Rust files were touched. Existing oversized files are listed under defects
  so later tasks can avoid growing them.

Error visibility notes:
- No error-handling code changed. Existing explicit test failures and provider
  configuration errors were preserved.

## Batch 03 - api-models-01 model metadata extraction

Status: WARNING
Commit: 4f303a9
Supplemental commit: d80cd9c
Tasks:
- [build] api-models-01 - Move model metadata into `cc-models`, rewire
  metadata-only callers, keep behavior unchanged, and run `cc-models` tests
  plus dependency checks.

Implemented effects:
- Added `cc-models` as a workspace crate for model aliases, provider model
  mappings, pricing metadata, and model display settings.
- Rewired `cc-bootstrap`, `cc-engine`, and `claude-code-rs` metadata-only
  callers to use `cc-models`.
- Removed root ownership of `model_registry` and `api::model_mapping`; kept
  `api::pricing` as a thin runtime helper because it still accepts
  `types::message::Usage`.
- Updated the batch file-size guard so historical oversized files warn when a
  batch does not grow them, while newly oversized files or further growth still
  block commit.
- Stabilized pricing tests that mutate `MODEL_INPUT_PRICE` and
  `MODEL_OUTPUT_PRICE` with test-only locks and environment snapshots.

Defects and divergences:
- Initial Batch 03 runner result was BLOCKER because the guard treated existing
  oversized files as hard failures even when this batch only rewired imports.
  The guard policy is now explicit and Batch 03 rechecks as warning-only.
- No behavior drift was found in model alias resolution, provider mapping, or
  pricing tests.
- Pricing environment overrides are process-global; tests now serialize only the
  env-mutating sections instead of requiring serial cargo invocation.

Follow-ups:
- api-models-02 review should verify `cc-models` remains dependency-free and
  does not gain transport/auth/request DTO ownership.
- Future edits to `api/client/mod.rs`, `api/vertex.rs`, and `main.rs` should be
  split-focused before adding behavior; those files remain historical size
  risks.
- Provider runtime behavior still needs real-credential validation in later API
  transport batches.

Verification:
- `cargo fmt --all --check`: pass.
- `cargo test -p cc-models`: pass, 28 passed.
- `cargo test -p claude-code-rs api::pricing`: pass, 2 passed.
- `cargo check -p claude-code-rs --message-format short`: pass.
- `cargo check --workspace --all-targets --message-format short`: pass.
- `cargo tree -p cc-models`: pass, no dependencies.
- `Invoke-DiffGuards` on Batch 03 changed files: warning-only after guard fix.

File-size/refactor findings:
- `crates/claude-code-rs/src/api/client/mod.rs` stayed at 1065 lines, above the
  800-line guard but not grown from HEAD.
- `crates/claude-code-rs/src/api/vertex.rs` stayed at 891 lines, above the
  800-line guard but not grown from HEAD.
- `crates/claude-code-rs/src/main.rs` decreased from 994 to 993 lines while
  remaining above the 800-line guard.
- Additional warning-only files: `cc-engine/src/status_line/payload.rs` 521
  lines, `api/bedrock.rs` 796 lines, `engine/agent/mod.rs` 668 lines, and
  `ui/components/command_surface/surfaces/config.rs` 566 lines.

Dependency graph notes:
- New dependency edges: `cc-bootstrap -> cc-models`, `cc-engine -> cc-models`,
  and `claude-code-rs -> cc-models`.
- `cc-models` has no dependencies and no transport/auth/runtime imports.
- `api::pricing` remains in `claude-code-rs` only as a bridge from
  `Usage` counters to `cc_models::get_pricing`.

Error visibility notes:
- Removed legacy model alias diagnostics remain explicit through
  `removed_legacy_model_alias_error`.
- Unknown pricing behavior remains explicit and test-covered as zero cost when
  env overrides are absent.
- File-size guard messages now include HEAD line-count context so later agents
  can distinguish historical size debt from batch-induced growth.

## Batch 05 - api-models-05 lane closeout

Status: PASS
Commit: not committed
Tasks:
- [final] api-models-05 - Remove api/model shims, run lane gates, update
  tracker, and commit only if all api/models checks are green.

Implemented effects:
- Removed the root `claude-code-rs` API compatibility shim
  (`crates/claude-code-rs/src/api.rs`) and the corresponding `mod api`
  declaration.
- Rewired remaining root-crate API consumers to `cc_api::api::*`.
- Preserved model metadata ownership in `cc-models`; no root model shim or
  `api::model_mapping` ownership remains.

Defects and divergences:
- None found in the API/models lane gates.
- The working tree contains unrelated pre-existing runner/supervisor changes;
  they were not modified for this closeout.

Follow-ups:
- Runner/owner: commit this batch if the surrounding runner policy requires it.
  This agent did not commit because the task-level contract says the runner owns
  commits.
- Future lanes should keep `cc-api` as the transport/request boundary and avoid
  reintroducing `crate::api::*` imports in `claude-code-rs`.

Verification:
- `cargo fmt --all --check`: pass.
- `cargo test -p cc-models`: pass, 28 passed.
- `cargo test -p cc-api`: pass, 126 passed.
- `cargo test -p claude-code-rs commands::login`: pass, 13 passed.
- `cargo test -p claude-code-rs engine::lifecycle`: pass, 49 passed.
- `cargo check -p claude-code-rs --message-format short`: pass.
- `cargo check --workspace --all-targets --message-format short`: pass.
- `cargo tree -p cc-models`: pass, no dependencies.
- `cargo tree -p cc-api`: pass; depends on `cc-auth`, `cc-models`,
  `cc-types`, and `cc-utils`.
- `rg` guard for `crate::api`, root `mod api`, root `model_registry`, and
  `api::model_mapping` under `crates/claude-code-rs/src`: pass, no matches.

File-size/refactor findings:
- No Rust file was grown structurally; edits were path rewrites plus shim
  deletion.
- Touched historical oversized files remain above guard thresholds:
  `engine/lifecycle/deps.rs` 2434 lines,
  `engine/lifecycle/submit_message.rs` 1405 lines,
  `query/loop_helpers.rs` 1343 lines, `main.rs` 992 lines, and
  `query/loop_impl.rs` 783 lines. No split was required because this closeout
  did not add behavior or increase those files.

Dependency graph notes:
- Removed edge: `claude-code-rs` no longer owns or exports a root `api` module.
- Remaining API dependency is explicit through the workspace crate edge
  `claude-code-rs -> cc-api`.
- `cc-models` remains dependency-free.

Error visibility notes:
- Provider configuration diagnostics remain explicit through
  `cc_api::api::client::ApiClient::from_backend_result`.
- Removed legacy model alias diagnostics remain explicit through `cc_models`.
- No redundant safety layer was added.

## Batch 06 - engine-query-00 checkpoint

Status: PASS
Commit: not committed
Tasks:
- [checkpoint] engine-query-00 - Inventory engine/query call sites, loop
  ownership, baseline tests, dependency graph, and file-size hot spots. No code
  moved.

Implemented effects:
- Documentation-only inventory added to this tracker.
- No Rust code, module declarations, imports, loop ownership, or crate
  dependencies were changed.

Loop ownership:
- Root `claude-code-rs` still owns the live query loop in
  `src/query/{loop_impl.rs,loop_helpers.rs,deps.rs,turn_context.rs}`.
- `QueryEngine::submit_message` in `engine/lifecycle/submit_message.rs` builds
  `QueryParams`, constructs `QueryEngineDeps`, calls `loop_impl::query(params,
  deps)`, then maps `QueryYield` into SDK-facing messages while updating
  session state.
- `QueryEngineDeps` in `engine/lifecycle/deps.rs` is the adapter from the query
  trait boundary into root runtime services: API calls, compaction,
  permissions, hooks, tool execution, MCP refresh, progress callbacks, and
  background-agent state.
- `cc-query` is still a scaffold crate only. It currently exports
  `CompletedBackgroundAgent` and `PendingBackgroundResults` from `cc-types` and
  does not own the loop implementation.
- `cc-engine` owns extracted status-line and engine shared type modules, but
  not the root `engine/` lifecycle or `query/` loop yet.

Call-site inventory:
- `submit_message` callers: web chat handlers, daemon proactive ticks, daemon
  gateway bridge, startup JSON/print modes, IPC query runner, TUI engine event
  driver, team runner, agent fork/dispatch/supervisor/worktree paths, and
  lifecycle tests.
- Direct `loop_impl::query` production caller: only
  `engine/lifecycle/submit_message.rs`.
- Direct `query(params, deps)` test callers live in `query/loop_tests.rs`.
- `QueryDeps` production implementation: `QueryEngineDeps`; test
  implementations are local mocks in query loop/helper tests.

Dependency graph notes:
- Current root edge: `claude-code-rs -> cc-engine` for status-line/types and
  `claude-code-rs -> cc-types` for shared background-agent/hook/command
  primitives.
- `claude-code-rs` does not depend on `cc-query` yet, so moving the loop will
  require adding that edge or rehoming via `cc-engine`.
- `cc-query -> cc-types` only; no API, tool, session, engine, UI, or transport
  dependencies are present in the scaffold crate.
- `cc-engine` currently depends on `cc-types`, `cc-config`, `cc-bootstrap`,
  `cc-models`, `cc-compact`, and `cc-keybindings`; it does not depend on
  `cc-api`, `cc-tools`, `cc-session`, or root `claude-code-rs`.
- Expected extraction pressure points: `QueryDeps` still references root-owned
  `QueryParams`, `QueryGates`, `QuerySource`, `ToolUseContext`, `Tools`,
  `AppState`, and message/result types through root re-exports or root modules.

Defects and divergences:
- None found by the targeted baseline tests.
- The worktree already contained unrelated modified Rust/script files before
  this checkpoint, including engine/query files; this task did not overwrite or
  normalize those changes.
- `cc-query` has no behavioral tests yet because it has no moved loop code.

Follow-ups:
- engine-query-01 should move query loop code only after choosing the exact
  homes for `QueryParams`, query DTOs, and `QueryDeps` request/result structs.
- Split or shrink touched oversized files before adding behavior to them,
  especially `engine/lifecycle/deps.rs`, `engine/lifecycle/submit_message.rs`,
  and `query/loop_helpers.rs`.
- Preserve `QueryEngineDeps` as the explicit runtime adapter; do not duplicate
  tool permission, hook, or result-size safety layers inside the loop move.

Verification:
- `cargo test -p cc-query`: pass, 0 tests.
- `cargo test -p cc-engine`: pass, 15 passed.
- `cargo test -p claude-code-rs query::`: pass, 40 passed.
- `cargo test -p claude-code-rs engine::lifecycle`: pass, 49 passed.
- `cargo tree -p cc-query`: pass; only workspace dependency is `cc-types`.
- `cargo tree -p cc-engine`: pass; no root-crate dependency.
- `cargo tree -p claude-code-rs -e normal --depth 1`: pass; shows existing
  `cc-engine` edge and no `cc-query` edge.

File-size/refactor findings:
- No Rust files were touched.
- Existing hot spots above guard thresholds: `query/loop_tests.rs` 2188 lines,
  `engine/lifecycle/deps.rs` 2221 lines,
  `engine/lifecycle/submit_message.rs` 1283 lines,
  `engine/system_prompt.rs` 1259 lines, `query/loop_helpers.rs` 1208 lines,
  `engine/agent/supervisor.rs` 969 lines,
  `engine/agent/worktree.rs` 749 lines, and `query/loop_impl.rs` 709 lines.
- Near-threshold files: `engine/agent/tool_impl.rs` 694 lines and
  `engine/agent/mod.rs` 598 lines.

Error visibility notes:
- No error-handling code changed.
- Existing explicit query-loop diagnostics remain covered by tests for stream
  start/stall errors, fallback exhaustion, prompt-too-long recovery,
  cancellation, hook failures, dangerous-command blocks, and tool panics.
- No redundant safety layer was added.

## Batch 07 - engine-query-06 lane closeout

Status: PASS
Commit: not committed
Tasks:
- [final] engine-query-06 - Remove engine/query shims, run lane gates, update
  tracker, and commit only if all engine/query checks are green.

Implemented effects:
- Removed the root `claude-code-rs` query compatibility shim
  (`crates/claude-code-rs/src/query/mod.rs`) and its `mod query` declaration.
- Rewired the remaining root-crate query import in
  `crates/claude-code-rs/src/safety/classifier.rs` to import
  `ModelCallParams` and `QueryDeps` directly from `cc_query`.
- Updated `cc-query` crate docs so they no longer describe a temporary
  `crate::query` shim.

Defects and divergences:
- None found in the engine/query lane gates.
- The worktree contains unrelated pre-existing runner/supervisor changes; this
  closeout did not modify them.

Follow-ups:
- Runner/owner: commit this batch if the surrounding runner policy requires it.
  This agent did not commit because the task-level contract says the runner owns
  commits.
- Future engine/query edits should keep root-crate callers importing from
  `cc_query` directly and avoid reintroducing `crate::query::*`.

Verification:
- `cargo fmt --all --check`: pass.
- `cargo test -p cc-query`: pass, 40 passed.
- `cargo test -p cc-engine`: pass, 17 passed.
- `cargo test -p claude-code-rs engine::lifecycle`: pass, 49 passed.
- `cargo check -p claude-code-rs --message-format short`: pass.
- `cargo check --workspace --all-targets --message-format short`: pass.
- `cargo tree -p cc-query`: pass.
- `rg` guard for root `crate::query`, root `mod query`, and temporary query
  shim text under `crates/claude-code-rs/src` and `crates/cc-query/src`: pass,
  no matches.

File-size/refactor findings:
- Deleted the root query shim file instead of growing it.
- Touched historical oversized files were not grown by behavior changes:
  `crates/claude-code-rs/src/main.rs` is 898 lines after removing `mod query`,
  and `crates/claude-code-rs/src/safety/classifier.rs` remains 712 lines with
  only an import-path rewrite.
- `crates/cc-query/src/lib.rs` is 16 lines.

Dependency graph notes:
- Removed the root module edge `claude-code-rs::query -> cc-query`.
- Remaining query-loop boundary is explicit through the workspace crate edge
  `claude-code-rs -> cc-query`.

Error visibility notes:
- No error-handling code changed.
- Existing explicit query-loop diagnostics remain covered by the `cc-query` and
  lifecycle tests listed above.
- No redundant safety layer was added.

## Batch 08 - tools-tasks-06 cc-tools slim specs and adapters

Status: WARNING
Commit: not committed
Tasks:
- [build] tools-tasks-06 - Slim cc-tools to specs, registry helpers, and thin
  adapters; rewire task callers.

Implemented effects:
- Replaced the `cc-tools` scaffold docs with a thin crate surface for pure
  registry helpers and task tool specs.
- Added `cc_tools::registry::ToolPolicy` plus policy allow-list helpers and
  rewired the root tool registry to re-export/use them.
- Added `cc_tools::task_specs` for task tool names, schemas, and task-output
  timeout bounds.
- Added a `tasks::tools()` adapter aggregator and rewired the root registry to
  call it instead of importing individual task tool structs.
- Rewired task tool adapters to source names/schemas from `cc-tools`; runtime
  behavior and validation remain in the root task module / `cc-tasks`.
- Added the root `claude-code-rs -> cc-tools` dependency and `cc-tools ->
  cc-tasks + serde_json` leaf dependencies.

Defects and divergences:
- `cargo test -p claude-code-rs tools::` compiled and ran the affected tests,
  but finished with 4 unrelated `tools::plan_mode` failures caused by a
  malformed external state file:
  `C:\Users\86186\.cc-rust\plan-workflow.json` (`trailing characters at line
  267 column 3`).
- The worktree already contained unrelated modified/deleted script files and
  untracked docs/scripts; this batch did not change them.

Follow-ups:
- Clean or isolate the global `~/.cc-rust/plan-workflow.json` test dependency
  before using broad `tools::` as a reliable lane gate on this machine.
- Future tools/tasks cleanup can continue moving runtime code into `cc-tasks`
  once the adapter dependencies are disentangled; keep `cc-tools` free of
  engine/query/UI/IPC/daemon edges.

Verification:
- `cargo fmt --all --check`: pass.
- `cargo check -p cc-tools --message-format short`: pass.
- `cargo test -p cc-tools`: pass, 3 passed.
- `cargo check -p cc-tasks --message-format short`: pass.
- `cargo check -p claude-code-rs --message-format short`: pass.
- `cargo test -p claude-code-rs tools::tasks --message-format short`: pass,
  58 passed.
- `cargo test -p claude-code-rs tools::registry --message-format short`: pass,
  7 passed.
- `cargo test -p claude-code-rs commands::tasks --message-format short`: pass,
  9 passed.
- `cargo tree -p cc-tools -e normal --depth 1`: pass; direct dependencies are
  only `cc-tasks` and `serde_json`.

File-size/refactor findings:
- Touched Rust files are under guard thresholds after the schema move:
  `tasks/task_tools.rs` 494 lines, `tasks.rs` 457, `cc-tools/src/task_specs.rs`
  251, `tools/registry.rs` 220, `cc-tools/src/registry.rs` 76.
- No new or moved Rust file exceeds 800 lines.

Dependency graph notes:
- `cc-tools` remains a leaf-style helper crate with no edge to
  `cc-engine`, `cc-query`, `cc-ipc`, `cc-daemon`, UI, or the root crate.

Error visibility notes:
- No task error paths were wrapped or duplicated.
- Existing task parsing/validation remains at the domain/runtime boundary; the
  adapter change only centralizes static specs and selection policy.

## Batch 09 - tools-tasks-07 lane closeout

Status: PASS
Commit: not committed
Tasks:
- [final] tools-tasks-07 - Delete task/tool shims, run lane gates, update
  tracker, and commit only if all tools/tasks checks are green.

Implemented effects:
- Deleted the unused root `tools::background_agents` compatibility shim and
  removed its module declaration.
- Removed public `tools::tasks` task-domain type re-exports; root callers now
  import task-domain types directly from `cc_tasks`.
- Kept task runtime/store/tool adapters in the root runtime module while
  preserving private internal imports for its submodules and tests.
- Updated `cc-tools` and `cc-tasks` crate descriptions so they no longer call
  the active boundary a scaffold.

Defects and divergences:
- None found in the tools/tasks lane gates.
- The worktree contains unrelated pre-existing script/supervisor changes and
  untracked docs/scripts; this closeout did not modify them.

Follow-ups:
- Runner/owner: commit this batch if the surrounding runner policy requires it.
  This agent did not commit because the task-level contract says the runner owns
  commits.
- Future tools/tasks moves should keep task-domain types imported from
  `cc_tasks` directly and avoid reintroducing root `tools::tasks` type
  re-export shims.

Verification:
- `cargo fmt --all --check`: pass.
- `cargo check -p cc-tools --message-format short`: pass.
- `cargo check -p cc-tasks --message-format short`: pass.
- `cargo test -p cc-tools`: pass, 3 passed.
- `cargo test -p cc-tasks`: pass, 10 passed.
- `cargo test -p claude-code-rs tools::tasks -- --nocapture`: pass, 58 passed.
- `cargo test -p claude-code-rs tools::registry -- --nocapture`: pass, 7 passed.
- `cargo test -p claude-code-rs commands::tasks -- --nocapture`: pass, 9 passed.
- `cargo check -p claude-code-rs --message-format short`: pass.
- `cargo check --workspace --all-targets --message-format short`: pass.
- `cargo tree -p cc-tools -e normal --depth 1`: pass; direct dependencies are
  only `cc-tasks` and `serde_json`.
- `cargo tree -p cc-tasks -e normal --depth 1`: pass; direct dependencies are
  `serde`, `serde_json`, and `tokio-util`.
- `rg` guard for root background-agent shim, root task-domain type re-exports,
  and workspace-split scaffold wording under the tools/tasks crates: pass, no
  matches.

File-size/refactor findings:
- Deleted the background-agent shim file instead of growing it.
- Touched Rust files stayed under the 800-line guard: `tasks/store.rs` 519
  lines, `tasks.rs` 406, `cc-tools/src/task_specs.rs` 240,
  `tools/registry.rs` 191, `cc-tools/src/registry.rs` 71, and
  `cc-tasks/src/lib.rs` 30.

Dependency graph notes:
- Removed root module surface: `claude-code-rs::tools::background_agents`.
- Removed public task-domain type shim surface from `tools::tasks`; remaining
  cross-boundary type dependency is explicit through `cc_tasks`.
- `cc-tools` remains a leaf-style helper crate with no edge to
  `cc-engine`, `cc-query`, `cc-ipc`, UI, daemon, or the root crate.

Error visibility notes:
- No task error paths were wrapped, hidden, or duplicated.
- Existing explicit task parsing/claim/persistence errors remain covered by the
  focused `cc-tasks` and `tools::tasks` tests.
- No redundant safety layer was added.
