# Anthropic API And Coding-Compatible Provider Risk Plan

Created: 2026-05-17
Mode: phased execution plan

## Goal

Bring cc-rust's Anthropic protocol support to a production-grade contract for
three different surfaces:

- Direct Anthropic API access through `https://api.anthropic.com`.
- Cloud Anthropic transports such as Bedrock, Vertex, and future Foundry.
- Third-party or internal "coding plan" deployments that expose an
  Anthropic-compatible Messages API shape through `ANTHROPIC_BASE_URL` and
  token-style auth, similar to `docs/reference/anthropic_coding.md`.

This plan intentionally does not treat Anthropic-compatible coding deployments
as OpenAI-compatible providers. They share the Anthropic Messages request and
streaming shape, so they need Anthropic protocol handling with provider-specific
auth, beta, caching, model, and capability rules.

## References

- `docs/reference/anthropic/api/overview.md`
- `docs/reference/anthropic/prompt_cache/overview.md`
- `docs/reference/anthropic_coding.md`
- `docs/mvp-optimization-plans/MVP-001-api-providers-plan.md`
- `docs/IMPLEMENTATION_GAPS.md`

## Current Risks

- `cc-auth` distinguishes API keys from bearer tokens, but
  `ApiProvider::Anthropic` stores only an `api_key` string. As a result,
  `ANTHROPIC_AUTH_TOKEN` and OAuth access tokens can be sent as `x-api-key`
  instead of `Authorization: Bearer ...`.
- Anthropic direct API, custom Anthropic-compatible base URLs, Bedrock, Vertex,
  and Foundry do not yet have a precise capability split for beta headers,
  prompt caching mode, token counting, request limits, and auth kind.
- `ANTHROPIC_BASE_URL` can route to third-party Anthropic-compatible endpoints,
  but the project does not expose a clear "coding plan" contract for that mode.
- Model default configuration still references upstream family names in places.
  cc-rust's public model policy should use `SOTA`, `MOTA`, and `FOTA`, with
  explicit provider model IDs behind those aliases.
- Prompt caching sends beta headers and records cache usage, but request bodies
  do not yet emit top-level or block-level `cache_control`.
- Streaming parser currently ignores Anthropic SSE `error` events.
- Real provider and mocked provider coverage still lags the capability matrix.

## Target Contract

### Auth

| Source | Provider mode | Header |
| --- | --- | --- |
| `ANTHROPIC_API_KEY` | Direct Anthropic API key | `x-api-key` |
| Keychain `cc-rust/api-key` | Direct Anthropic API key | `x-api-key` |
| `ANTHROPIC_AUTH_TOKEN` | Bearer/coding-compatible token | `Authorization: Bearer ...` |
| OAuth access token | Bearer token | `Authorization: Bearer ...` |
| Bedrock | Bedrock auth adapter | Bedrock bearer or SigV4 |
| Vertex | Vertex auth adapter | Google bearer token |

### Model Defaults

Canonical cc-rust model tiers:

- `SOTA`: highest capability tier.
- `MOTA`: balanced default tier.
- `FOTA`: fast tier.

Planned canonical environment variables:

- `ANTHROPIC_DEFAULT_SOTA_MODEL`
- `ANTHROPIC_DEFAULT_MOTA_MODEL`
- `ANTHROPIC_DEFAULT_FOTA_MODEL`

Compatibility-only fallback variables:

- `ANTHROPIC_DEFAULT_OPUS_MODEL` maps to `SOTA`.
- `ANTHROPIC_DEFAULT_SONNET_MODEL` maps to `MOTA`.
- `ANTHROPIC_DEFAULT_HAIKU_MODEL` maps to `FOTA`.

The fallback variables should remain readable for migration, but docs and new
examples should use the SOTA/MOTA/FOTA names. Removed public aliases `opus`,
`sonnet`, and `haiku` must stay rejected at user/config entry points.

### Anthropic-Compatible Coding Mode

The coding-compatible mode is active when a custom Anthropic protocol endpoint
is configured, for example:

```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "provider-token",
    "ANTHROPIC_BASE_URL": "https://provider.example.com/anthropic",
    "ANTHROPIC_DEFAULT_SOTA_MODEL": "provider-coding-pro",
    "ANTHROPIC_DEFAULT_MOTA_MODEL": "provider-coding-main",
    "ANTHROPIC_DEFAULT_FOTA_MODEL": "provider-coding-fast",
    "ANTHROPIC_MODEL": "MOTA"
  }
}
```

The provider must be treated as Anthropic protocol if it expects `/v1/messages`
and Anthropic SSE events. It should not be routed through
`/chat/completions`.

## Phase 0 - Baseline And Fixtures

Goal: freeze current behavior before changing provider contracts.

Work:

- Inventory existing tests in:
  - `crates/cc-api/src/api/client/tests.rs`
  - `crates/cc-api/src/api/streaming.rs`
  - `crates/cc-auth/src/lib.rs`
  - `crates/cc-models/src/aliases.rs`
  - `crates/cc-engine/src/lifecycle/helpers.rs`
- Add mocked HTTP fixtures for:
  - Anthropic API key headers.
  - Anthropic bearer headers.
  - Custom `ANTHROPIC_BASE_URL` request URL.
  - Count-token request headers.
  - Streaming `error` SSE event.
  - Prompt-cache request bodies.
- Record existing failures or compile blockers before implementation.

Verification:

- `cargo test -p cc-auth --lib`
- `cargo test -p cc-models --lib`
- `cargo test -p cc-api api::client -- --nocapture`
- `cargo test -p cc-api api::streaming -- --nocapture`

Exit criteria:

- The next phases have failing or ignored regression tests that prove the risk.
- The baseline documents current header, model, cache, and stream behavior.

## Phase 1 - Preserve Anthropic Auth Kind End To End

Goal: send official Anthropic auth headers correctly for API keys and bearer
tokens.

Work:

- Replace `ApiProvider::Anthropic { api_key, base_url }` with a structure that
  stores auth kind, for example `AnthropicAuth::ApiKey(String)` and
  `AnthropicAuth::Bearer(String)`.
- Preserve `AuthMethod` kind when building `ApiClient` in `from_auth_result`.
- Update non-streaming Messages, streaming Messages, and count-token request
  builders to use a shared Anthropic header builder.
- Keep `anthropic-version` and capability-selected `anthropic-beta` in the same
  helper so header snapshot tests cover all Anthropic request paths.
- Ensure request snapshots sanitize headers and never persist credential values.

Targeted tests:

- `ANTHROPIC_API_KEY` sends `x-api-key` and does not send `Authorization`.
- `ANTHROPIC_AUTH_TOKEN` sends `Authorization: Bearer ...` and does not send
  `x-api-key`.
- OAuth bearer auth follows the bearer path.
- Count-token and streaming requests use the same auth-kind behavior.
- Invalid direct Anthropic API keys still fail fast when using
  `ANTHROPIC_API_KEY`.

Verification:

- `cargo test -p cc-auth --lib`
- `cargo test -p cc-api api::client -- --nocapture`
- `cargo test -p cc-api api::stream_provider -- --nocapture`

Exit criteria:

- No bearer token can be serialized as `x-api-key` on an Anthropic protocol
  request.

## Phase 2 - Add Anthropic-Compatible Coding Provider Contract

Goal: support third-party Anthropic-format coding deployments as a first-class
Anthropic protocol mode.

Work:

- Add a provider capability profile for custom Anthropic-compatible endpoints.
  It should be distinct from direct Anthropic, Bedrock, Vertex, OpenAI-compatible
  providers, and unsupported Foundry.
- Detect this mode when `ANTHROPIC_BASE_URL` is set to a non-Anthropic host or
  when an explicit opt-in flag is introduced.
- Allow bearer-token auth for this mode through `ANTHROPIC_AUTH_TOKEN`.
- Decide whether non-`sk-ant-` API keys are allowed only in this mode. Direct
  Anthropic API keys should keep format validation.
- Add config/status diagnostics that show the provider as
  `anthropic-compatible` or similar, not as direct Anthropic.
- Route requests to `{base_url}/v1/messages` and parse Anthropic SSE events.
- Keep provider capability flags explicit:
  - tool use.
  - thinking.
  - exact token counting.
  - prompt cache explicit.
  - prompt cache automatic.
  - advisor.
  - request size limit.

Targeted tests:

- `ANTHROPIC_BASE_URL=https://provider.example/anthropic` routes to
  `https://provider.example/anthropic/v1/messages`.
- Custom compatible mode accepts bearer auth without `sk-ant-` validation.
- OpenAI-compatible providers still route to `/chat/completions`.
- Unsupported capabilities are omitted from beta headers and request bodies.
- Status output identifies the custom provider mode clearly.

Verification:

- `cargo test -p cc-api api::client -- --nocapture`
- `cargo test -p cc-api api::providers -- --nocapture`
- `cargo test -p cc-commands login -- --nocapture`

Exit criteria:

- A user can configure an Anthropic-format coding endpoint with
  `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, and SOTA/MOTA/FOTA model
  defaults without sending an OpenAI-style request.

## Phase 3 - Move Family Defaults To SOTA/MOTA/FOTA

Goal: make the model-default surface match cc-rust's neutral public aliases.

Work:

- Add canonical env readers for:
  - `ANTHROPIC_DEFAULT_SOTA_MODEL`
  - `ANTHROPIC_DEFAULT_MOTA_MODEL`
  - `ANTHROPIC_DEFAULT_FOTA_MODEL`
- Keep fallback support for upstream family variables:
  - `ANTHROPIC_DEFAULT_OPUS_MODEL`
  - `ANTHROPIC_DEFAULT_SONNET_MODEL`
  - `ANTHROPIC_DEFAULT_HAIKU_MODEL`
- Define precedence:
  1. CLI `--model`.
  2. config `model`.
  3. `ANTHROPIC_MODEL`.
  4. selected alias default from SOTA/MOTA/FOTA env map.
  5. built-in cc-rust alias target.
- When `ANTHROPIC_MODEL` is `SOTA`, `MOTA`, or `FOTA`, resolve it through the
  provider model map before sending the request.
- Reject legacy public aliases `opus`, `sonnet`, and `haiku` before request
  construction.
- Update:
  - `docs/reference/anthropic_coding.md`.
  - `docs/claude-code-configuration/model-configuration.md`.
  - `docs/COMMAND_REFERENCE.md`.
  - `/model` and `/config show` output if they mention old family defaults.

Targeted tests:

- `SOTA` resolves through `ANTHROPIC_DEFAULT_SOTA_MODEL` when set.
- `MOTA` resolves through `ANTHROPIC_DEFAULT_MOTA_MODEL` when set.
- `FOTA` resolves through `ANTHROPIC_DEFAULT_FOTA_MODEL` when set.
- Old `OPUS/SONNET/HAIKU` env vars still work as fallback and emit a migration
  diagnostic.
- Legacy user-facing aliases remain rejected.
- `availableModels` accepts aliases and provider model IDs as documented.

Verification:

- `cargo test -p cc-models --lib`
- `cargo test -p cc-api api::client -- --nocapture`
- `cargo test -p cc-commands model -- --nocapture`
- `cargo test -p claude-code-rs --bin claude-code-rs startup -- --nocapture`

Exit criteria:

- New docs and examples use SOTA/MOTA/FOTA names.
- Provider requests never receive removed aliases unless a user explicitly
  supplied a full provider model ID with that literal name.

## Phase 4 - Implement Prompt Cache Request Bodies

Goal: turn prompt caching from header-only support into real request-body
support.

Work:

- Add `CacheControl` types for:
  - `type: ephemeral`.
  - optional `ttl: 1h`.
- Add top-level `cache_control` to `MessagesRequest`.
- Add optional per-block `cache_control` to system text blocks, tool definitions,
  and message content blocks.
- Wire `skip_cache_write` into request construction. If the provider has no
  read-only cache mode, omit cache-control markers and document that behavior.
- Add provider capability checks:
  - direct Anthropic: automatic and explicit cache allowed.
  - Anthropic-compatible coding mode: controlled by provider capability.
  - Bedrock and Vertex: no automatic caching unless docs change.
  - Foundry: remain unsupported until adapter exists.
- Preserve cache usage fields in streaming and non-streaming accumulation.
- Add request snapshot redaction that preserves cache-control shape.

Targeted tests:

- Automatic caching adds top-level `cache_control`.
- Explicit caching places block-level `cache_control` on selected content.
- Bedrock and Vertex reject or omit automatic cache mode.
- `skip_cache_write` removes cache write markers.
- Cache usage is exported in session snapshots.

Verification:

- `cargo test -p cc-engine lifecycle::helpers -- --nocapture`
- `cargo test -p cc-api api::client -- --nocapture`
- `cargo test -p cc-session session_export -- --nocapture`

Exit criteria:

- Prompt-cache behavior is visible in serialized request fixtures, not just in
  headers.

## Phase 5 - Stream Error And Messages Surface Hardening

Goal: prevent Anthropic protocol drift and hidden runtime failures.

Work:

- Parse Anthropic SSE `error` events into structured errors.
- Preserve provider request IDs from error responses when headers are present.
- Add typed support or explicit omissions for modern Messages fields:
  - `metadata`.
  - `service_tier`.
  - `stop_sequences`.
  - `temperature`, `top_p`, `top_k`.
  - `container`, `context_management`, and `mcp_servers` only if the project
    decides to support those managed-agent style features.
- Keep unsupported fields out of request bodies rather than serializing
  provider-incompatible placeholders.
- Extend retry categorization for Anthropic error payloads.

Targeted tests:

- SSE `error` event fails the stream with provider error details.
- `ping` remains ignored.
- Missing required SSE fields still fail parse.
- Optional Messages fields serialize only when set.
- Unsupported provider capability prevents unsupported fields from being sent.

Verification:

- `cargo test -p cc-api api::streaming -- --nocapture`
- `cargo test -p cc-api api::client -- --nocapture`
- `cargo test -p cc-engine lifecycle::deps -- --nocapture`

Exit criteria:

- Anthropic protocol errors cannot disappear as empty stream events.

## Phase 6 - Provider Smoke Matrix And Documentation Closure

Goal: close the gap between capability claims and real behavior.

Work:

- Add mock-server tests for direct Anthropic, Anthropic-compatible coding,
  Bedrock, Vertex, and unsupported Foundry.
- Add opt-in real smoke scripts gated by credentials:
  - direct Anthropic API key.
  - direct Anthropic bearer token if available.
  - custom Anthropic-compatible endpoint.
  - Bedrock.
  - Vertex.
- Update docs when each provider passes:
  - `docs/IMPLEMENTATION_GAPS.md`
  - `docs/WORK_STATUS.md`
  - `docs/KNOWN_ISSUES.md`
  - `docs/archive/COMPLETED_FULL.md`
- Keep any intentionally unsupported provider behavior marked as
  `Intentional`, not as a silent omission.

Verification:

- `cargo build --workspace --release`
- Targeted provider unit tests from earlier phases.
- Real smoke scripts only when credentials are present.

Exit criteria:

- The provider capability matrix, docs, and tests agree.
- Anthropic-compatible coding setup has a documented, tested path using
  SOTA/MOTA/FOTA defaults.

## Recommended Implementation Order

1. Phase 1 auth-kind fix.
2. Phase 2 Anthropic-compatible coding provider contract.
3. Phase 3 SOTA/MOTA/FOTA default model envs.
4. Phase 4 prompt-cache request body support.
5. Phase 5 stream error and optional Messages field hardening.
6. Phase 6 smoke matrix and documentation closure.

Auth-kind correctness should be first because it affects every Anthropic
protocol request and can break both direct Anthropic API and compatible coding
deployments. SOTA/MOTA/FOTA model defaults should land before broad coding-mode
documentation so new examples do not teach the old OPUS/SONNET/HAIKU variables.
