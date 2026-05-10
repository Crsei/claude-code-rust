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
