# Context Implementation Phase Plan

Created: 2026-05-06
Source map: `architecture/context-implementation-map.md`

## Phase Progress

- Phase 0 - Baseline And Decisions: completed on 2026-05-06.
  - Decision artifact: `docs/archive/context-phase0-decisions-2026-05-06.md`.
  - New regression lock: `test_context_maps_are_metadata_not_prompt_sections` in `crates/claude-code-rs/src/engine/system_prompt.rs`.
  - Verification: targeted baseline tests for token usage, memory context, session-insights filtering, system prompt context metadata, and compaction pipeline.
- Phase 1 - Exact Token Counting Path: completed on 2026-05-06.
  - Provider matrix: `docs/archive/context-phase1-token-count-provider-matrix-2026-05-06.md`.
  - Added `TokenCountMethod::ProviderExact`, provider metadata on `TokenUsageReport`, and an async exact-count path for Anthropic/Azure/Gemini.
  - Exact count is currently diagnostics-only behind `CC_RUST_EXACT_TOKEN_DIAGNOSTICS=1`; auto-compact remains heuristic until the near-threshold fallback tests are implemented.
  - Verification: targeted cc-utils token report test plus Anthropic body-shape, provider support matrix, and Gemini countTokens body-shape tests.
- Phase 2 - Relevant Memory Recall: pending.
- Phase 3 - Compaction Parity And Partial Compact: pending.
- Phase 4 - End-To-End Context Verification: pending.

## Requirements Summary

This plan turns the `context` implementation map's `部分实现` / `未实现` / `待确认` items into phase-level implementation work. It intentionally excludes `system-prompt.mdx` as primary feature work because the map marks static/dynamic sections, cache boundary, `CLAUDE.md` injection, and append/override ordering as implemented (`architecture/context-implementation-map.md:31`, `architecture/context-implementation-map.md:92`).

In scope:

- Complete `token-budget.mdx` parity where cc-rust currently reports heuristic usage only, with `TokenCountMethod::Heuristic` and `exact_count_available=false` (`architecture/context-implementation-map.md:32`, `architecture/context-implementation-map.md:111`, `crates/cc-utils/src/tokens.rs:37`).
- Complete `project-memory.mdx` parity for intelligent recall, recent-tool denoising, and already-surfaced dedupe; current CRUD, `MEMORY.md` indexing, scoped injection, auto memory, and session-insights are already present (`architecture/context-implementation-map.md:30`, `architecture/context-implementation-map.md:68`, `architecture/context-implementation-map.md:136`).
- Complete `compaction.mdx` parity for explicit feature-gate semantics, Partial Compact, and recovery semantics; current pipeline, boundaries, Session Memory Compact, model-summary compact, snip/context-collapse metadata, PTL handling, and hooks already exist (`architecture/context-implementation-map.md:29`, `architecture/context-implementation-map.md:44`, `architecture/context-implementation-map.md:135`).
- Resolve the `待确认` item for `user_context` / `system_context`: decide whether these are intentionally retained metadata or should become an independent API input layer (`architecture/context-implementation-map.md:141`).

Out of scope:

- Rewriting the already implemented system prompt builder.
- Adding unrelated context features from `agent`, `extensibility`, `safety`, or `tools`.
- Declaring any context gap as intentional crop without an explicit follow-up decision; the map currently found no proven intentional crop items (`architecture/context-implementation-map.md:148`).

## Acceptance Criteria

- `TokenUsageReport` can represent provider-level exact counts when available and keeps a deterministic heuristic fallback when exact counting is unavailable or disabled.
- Auto-compact and context diagnostics use exact counts only where latency and provider support make it safe; hot local paths still work offline.
- Memory recall can select at most 5 relevant memories using current query context, avoid recently used tool-reference noise, and avoid re-surfacing the same memory within a session unless explicitly needed.
- Memory injection preserves existing Project / Global / Team / Auto behavior and session-insights filtering.
- Partial Compact supports `up_to` and `from` directions in core compaction logic, preserves API message invariants, records boundary metadata, and has PTL retry behavior.
- Resume/export/load behavior after manual compact, auto compact, Session Memory Compact, context collapse, snip, and Partial Compact is covered by regression tests.
- `build_system_prompt()`'s `user_context` / `system_context` role is documented and either serialized intentionally or explicitly kept as metadata with tests.
- `cargo test` passes for affected crates, followed by `cargo build --release` with no new warnings.
- Relevant docs move completed gaps out of `architecture/context-implementation-map.md` follow-up status and into the completed/full-build tracker.

## Phase 0 - Baseline And Decisions

Goal: lock current behavior before changing context semantics.

Work:

- Add or identify regression tests for the existing baseline:
  - heuristic `TokenUsageReport` shape in `crates/cc-utils/src/tokens.rs`;
  - memory scope/index/context assembly in `crates/cc-session/src/memdir.rs`;
  - session-insights filtering in `crates/cc-services/src/session_memory.rs`;
  - prompt memory injection in `crates/claude-code-rs/src/engine/system_prompt.rs`;
  - compaction pipeline ordering and metadata in `crates/cc-compact/src/pipeline.rs`, `snip.rs`, `context_collapse.rs`, `microcompact.rs`, and `compaction.rs`.
- Decide the exact-count abstraction boundary:
  - keep `cc-utils` as the sync/offline heuristic estimator;
  - put provider-backed exact counting behind API/client or engine-layer async interfaces to avoid making `cc-utils` depend on network/provider code.
- Decide the `user_context` / `system_context` contract:
  - Option A: serialize them as separate input layers if the provider request model supports it;
  - Option B: document them as internal provenance metadata and add tests proving only `system_prompt`, messages, and tools are serialized.
- Record provider-doc requirements for implementation. Exact token APIs are provider-specific, so implementation must check official provider docs before wiring Anthropic / Google / OpenAI-compatible paths.

Exit criteria:

- Current partial behavior is covered by tests or explicitly documented test gaps.
- The exact-token boundary and `user_context` / `system_context` decision are written into the implementation PR/ADR.
- No feature implementation starts until this phase has a clear decision artifact.

## Phase 1 - Exact Token Counting Path

Goal: close the only `未实现` item in the context map: provider-level exact token counting.

Work:

- Extend `TokenCountMethod` and `TokenUsageReport` so reports can distinguish heuristic vs provider exact counts while preserving current fields for compatibility (`crates/cc-utils/src/tokens.rs:37`).
- Add an async exact-count interface outside `cc-utils`, likely near provider request construction in `crates/claude-code-rs/src/api/**` and `crates/claude-code-rs/src/engine/lifecycle/helpers.rs`.
- Normalize messages, system prompt, tools, and model options through the same request-shaping path used by `build_messages_request()` so exact counts match the real request surface.
- Use exact counts selectively:
  - context diagnostics and `/context`-style reporting should prefer exact count when available;
  - auto-compact should ask for exact count only when the heuristic estimate is near threshold, avoiding network cost on clearly safe inputs;
  - fallback to heuristic on unsupported providers, offline mode, provider errors, or cancellation.
- Preserve existing dynamic context-window parsing from `CLAUDE_CODE_MAX_CONTEXT_TOKENS` and `[1m]` model suffixes.

Tests:

- Unit tests for `TokenCountMethod` / `TokenUsageReport` compatibility and fallback.
- Mock-provider tests proving exact count success sets `exact_count_available=true`.
- Mock-provider tests proving provider failure falls back to heuristic without blocking the query loop.
- Threshold tests proving exact count can prevent both false positive and false negative auto-compact decisions near the threshold.

Exit criteria:

- The map's `未实现` token-count item can be reclassified from `未实现` to `已实现` or `部分实现` with a clear provider matrix.

## Phase 2 - Relevant Memory Recall

Goal: close the `project-memory.mdx` parity gaps without regressing existing memory injection.

Work:

- Introduce a relevant-memory lookup API in `cc-session::memdir` or a new adjacent module:
  - scan memory entries and `MEMORY.md` index material;
  - include optional metadata fields needed for relevance, such as description/search terms, while maintaining backward compatibility with existing JSON entries;
  - return a bounded list, default max 5, matching the Bun recall shape.
- Add a model-assisted side-query path using the existing API client where available, plus a deterministic keyword/scoring fallback for offline and test runs.
- Pass query text, recently used tool names, and an in-session `already_surfaced` set into recall.
- Implement recent-tool denoising:
  - skip generic usage docs for tools that are already active in the current work context;
  - still allow warnings, pitfalls, and known issues for those tools.
- Track surfaced memory IDs/paths in the session state so repeated turns do not spend the recall budget on the same memory unless the user explicitly asks for it.
- Add prompt guidance equivalent to "Before recommending from memory" and "ignore memory" semantics, but keep it integrated with the current `# Memory Context` section rather than rewriting the whole prompt builder.

Tests:

- Unit tests for scanning, scoring, max-5 selection, tool denoising, and already-surfaced filtering.
- Tests proving legacy entries without new optional metadata still load.
- Prompt assembly tests proving existing Project / Global / Team / Auto memory and session-insights still appear under the correct gates.
- Tests for "ignore memory" behavior at prompt-instruction level.

Exit criteria:

- The map's `project-memory.mdx` item can be narrowed to only deliberately deferred advanced modes, or reclassified to `已实现` for the documented Bun semantics covered here.

## Phase 3 - Compaction Parity And Partial Compact

Goal: close `compaction.mdx` gaps around feature gates, Partial Compact, and full recovery semantics.

Work:

- Make compaction feature gates explicit in Rust config/state:
  - Session Memory Compact enablement;
  - reactive compact enablement;
  - Partial Compact enablement;
  - auto-compact disable/circuit-breaker behavior.
- Add core Partial Compact primitives:
  - `up_to(anchor)` compresses content before the selected message and preserves recent context;
  - `from(anchor)` compresses content after the selected message and preserves early context/cache prefix;
  - both directions preserve tool-use/tool-result invariants and thinking/tool-use grouped message invariants.
- Wire Partial Compact into the command/API layer after the core library is tested. UI selection can be a separate surface if no existing selection command exists.
- Strengthen recovery semantics:
  - boundary metadata must include enough preserved-segment information to reconstruct the post-compact chain;
  - session export/load and resume should start from the latest effective compact boundary and avoid reintroducing stale pre-compact usage;
  - manual compact after reactive/partial compact must not reuse stale preserved segments.
- Keep existing local pipeline order unless tests prove it must change:
  - tool result budget;
  - snip;
  - microcompact;
  - context collapse;
  - auto compact (`crates/cc-compact/src/pipeline.rs:83`).

Tests:

- Unit tests for `up_to` and `from` anchor behavior.
- API-invariant tests for preserved tool pairs and split assistant messages.
- PTL retry tests that drop oldest API turns while retaining at least one summarizable group.
- Session export/load round-trip tests for manual, auto, Session Memory Compact, context collapse, snip, and Partial Compact.
- Regression tests proving current microcompact and boundary metadata remain intact.

Exit criteria:

- `compaction.mdx` no longer has unresolved feature-gate / Partial Compact / recovery semantics gaps in the map.

## Phase 4 - End-To-End Context Verification

Goal: prove the three completed lanes work together in realistic query flow.

Work:

- Add e2e or integration fixtures that exercise:
  - memory recall + prompt injection + exact token reporting;
  - near-threshold exact token count + auto-compact;
  - Session Memory Compact + resume;
  - Partial Compact + resume/export;
  - `user_context` / `system_context` contract from Phase 0.
- Run the required verification sequence:
  - targeted crate tests while developing;
  - broader `cargo test`;
  - `cargo build --release`;
  - warning cleanup.
- Update docs:
  - `architecture/context-implementation-map.md`;
  - `docs/WORK_STATUS.md` if this becomes a tracked full-build milestone;
  - `docs/IMPLEMENTATION_GAPS.md` only if a remaining context behavior is intentionally retained as a crop.

Exit criteria:

- All context map gaps are either implemented, explicitly reclassified with evidence, or intentionally cropped with documented rationale and review conditions.
- No new warnings remain after release build.

## Recommended Order

1. Phase 0 first, because it prevents reworking exact-count and prompt-context decisions.
2. Phase 1 next, because exact counts influence compaction thresholds and context diagnostics.
3. Phase 2 next, because memory recall feeds both normal prompt context and Session Memory Compact.
4. Phase 3 after token/memory foundations, because compaction correctness depends on both.
5. Phase 4 last, as the acceptance gate for cross-module behavior and documentation closure.

## Risks And Mitigations

- Provider exact token APIs may differ or be unavailable. Mitigation: provider matrix plus heuristic fallback; exact count is an enhancement, not a hard dependency.
- Exact counting can add latency near every turn. Mitigation: only call exact count near thresholds or when diagnostics explicitly request it.
- Memory recall can inject stale or noisy facts. Mitigation: retain current scope/time/tag filters, add recent-tool denoising, already-surfaced filtering, and prompt guidance to verify file/function claims.
- Partial Compact can corrupt message invariants. Mitigation: implement library-level invariant tests before command/UI wiring.
- Resume/export behavior can regress silently. Mitigation: JSONL/session round-trip tests with compact boundaries and preserved segments.

## Verification Plan

- Phase 0: existing behavior regression tests and decision artifact.
- Phase 1: token unit tests, mocked provider exact/fallback tests, near-threshold auto-compact tests.
- Phase 2: memory scan/selection tests, prompt assembly tests, legacy entry compatibility tests.
- Phase 3: compaction direction tests, API invariant tests, PTL retry tests, session export/load round-trip tests.
- Phase 4: full integration/e2e suite plus `cargo test` and `cargo build --release`.
