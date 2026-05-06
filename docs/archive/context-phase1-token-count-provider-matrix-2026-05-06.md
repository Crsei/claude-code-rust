# Context Phase 1 Token Count Provider Matrix

Date: 2026-05-06
Plan: `.omx/plans/context-implementation-phase-plan-2026-05-06.md`

## Decision

Phase 1 adds an async exact-token-count path outside `cc-utils`.
`cc-utils` remains the synchronous offline heuristic layer; provider-backed
callers can convert exact counts into the same `TokenUsageReport` shape with
`TokenCountMethod::ProviderExact`.

The exact path is currently exposed as diagnostics, gated by
`CC_RUST_EXACT_TOKEN_DIAGNOSTICS=1`, so normal query traffic keeps the existing
latency profile. Auto-compact still uses the deterministic heuristic threshold
until near-threshold exact-count fallback is wired and tested separately.

## Provider Matrix

| Provider | Phase 1 status | Endpoint shape | Notes |
| --- | --- | --- | --- |
| Anthropic | Supported | `POST /v1/messages/count_tokens` | Uses the same shaped messages request body minus generation-only fields (`stream`, `max_tokens`, `advisor_model`). Parses `input_tokens`. |
| Azure Anthropic-compatible | Supported | `POST {endpoint}/v1/messages/count_tokens` | Uses the Anthropic-compatible body/header path against the configured Azure endpoint. |
| Google Gemini | Supported | `POST {base}/models/{model}:countTokens?key=...` | Wraps the normal Gemini `generateContent` body as `generateContentRequest` and parses `totalTokens`. |
| OpenAI / OpenAI-compatible | Heuristic fallback | No cc-rust exact preflight endpoint wired | Official OpenAI guidance remains tokenizer/usage based for preflight counting, so this phase keeps OpenAI-compatible providers on the heuristic path. |
| Bedrock | Heuristic fallback | Not wired | Requires separate provider-specific verification before implementation. |
| Vertex | Heuristic fallback | Not wired | Requires separate Vertex/Gemini or Anthropic-on-Vertex endpoint verification before implementation. |

## Official References Checked

- Anthropic Message Count Tokens API: https://docs.anthropic.com/en/api/messages-count-tokens
- Anthropic token counting guide: https://docs.anthropic.com/en/docs/build-with-claude/token-counting
- Gemini token counting API: https://ai.google.dev/api/tokens
- OpenAI token concepts and tokenizer guidance: https://platform.openai.com/docs/concepts/tokens

## Verification

- `cargo test -p cc-utils test_token_usage_report_from_provider_count_marks_exact`
- `cargo test -p claude-code-rs test_anthropic_count_tokens_body_omits_generation_only_fields`
- `cargo test -p claude-code-rs test_exact_token_count_support_matrix`
- `cargo test -p claude-code-rs test_build_gemini_count_tokens_request_wraps_generate_content_request`
