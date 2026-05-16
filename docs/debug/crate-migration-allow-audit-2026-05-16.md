# Crate Migration Allow Audit

> Date: 2026-05-16
> Scope: all Rust source files under this repository (`*.rs`)
> Purpose: record every `allow(dead_code)` and `allow(unused_imports)` left visible after the crate migration / owner migration work.

This file is a debug inventory only. It does not decide which allowances are valid long term; it records the current state so the thin-binary closeout can remove, justify, or convert each item.

## Scan Commands

```bash
rg --glob '*.rs' 'allow\([^)]*dead_code'
rg --glob '*.rs' 'allow\([^)]*unused_imports'
```

Cross-checks:

```bash
rg -n '#!?\[allow\([^\]]*\bdead_code\b' -g '*.rs'
rg -n '#!?\[allow\([^\]]*\bunused_imports\b' -g '*.rs'
rg -n '#!?\[allow\([^\]]*\bdead_code\b[^\]]*,|#!?\[allow\([^\]]*,[^\]]*\bdead_code\b' -g '*.rs'
rg -n '#!?\[allow\([^\]]*\bunused_imports\b[^\]]*,|#!?\[allow\([^\]]*,[^\]]*\bunused_imports\b' -g '*.rs'
```

Notes:

- The wider `allow\([^)]*dead_code` scan includes `cfg_attr(..., allow(dead_code))`.
- This inventory intentionally excludes broad `allow(unused)` entries; those are a separate cleanup class.
- Two read-only explorer subagents independently checked `dead_code` and `unused_imports`; the local scan matched their results.

## Summary

| Allowance | Total hits | Files | Direct standalone | Conditional standalone | Compound |
| --- | ---: | ---: | ---: | ---: | ---: |
| `allow(dead_code)` | 363 | 94 | 362 | 1 | 0 |
| `allow(unused_imports)` | 16 | 7 | 16 | 0 | 0 |

Top concentration:

- `crates/claude-code-rs/src/ui/**`: 261 `dead_code` hits and 1 `unused_imports` hit. These are mostly module re-export/path-shim allowances around the root-owned Rust TUI surface.
- `crates/claude-code-rs/tests/**`: 2 `dead_code` hits.
- `crates/claude-code-rs/src/app_subsystem_handlers.rs`: 1 `dead_code` hit.
- `crates/cc-*`, `crates/voice`, and `crates/worktree`: 99 `dead_code` hits, including 1 conditional `cfg_attr`, and 15 `unused_imports` hits.

## First Batch Classification: `cc-api` and `cc-engine`

Classification rules used for this batch:

| Category | Decision rule |
| --- | --- |
| Stale allow | Remove the allow when current runtime code or tests already use the symbol. |
| Public API / re-export | Keep the API surface, remove unnecessary allow, and add focused tests where useful. |
| Test-only helper | Move to `#[cfg(test)]` or a test module and remove the production allow. |
| Missing implementation | Keep the symbol if it represents upstream parity or planned wiring; retain only a narrow documented allow if Rust still warns. |
| Intentional reserve | Keep the symbol without an allow when possible; otherwise document the narrow allow. |
| Truly unused | Remove private code only when it has no callers, tests, or parity value. |

| Area | Original allow sites | Category | Evidence | Decision |
| --- | --- | --- | --- | --- |
| `cc-api` retry | `RetryConfig`, `ApiErrorCategory` variants, `is_retryable`, `retry_delay`, `rand_fraction` | Stale allow | `ApiClient::messages_stream_with_backoff` constructs `RetryConfig`, calls `is_retryable`, and calls `retry_delay`; retry tests cover stream-start retryable/non-retryable cases. | Removed all retry `dead_code` allows. |
| `cc-api` provider metadata | `ProviderInfo.label`, `get_provider`, `available_providers` | Public API / re-export | `get_provider` is used by capability lookup and client construction; provider tests now assert label metadata. `available_providers` is public diagnostics/config surface. | Removed allows and kept public API. |
| `cc-api` client helpers | `Azure`, OpenAI-compatible `default_model`, `ApiClientConfig.max_retries`, `build_url`, `from_codex_auth`, `build_headers`, `build_headers_map`, `messages` | Stale allow / Public API | URL helpers, auth helpers, and header-map helpers are covered by existing tests; added direct `HeaderMap` coverage and synthetic-stream `messages()` collection coverage. `max_retries` is used by stream retry config. | Removed all client helper allows. |
| `cc-api` SSE text parser | `parse_sse_text` | Test-only helper | Only client tests use the text parser; runtime uses `parse_sse_byte_stream`. | Changed to `#[cfg(test)]`; removed production allow. |
| `cc-api` Gemini response role | `GeminiContent.role` | Intentional reserve | Gemini may send `content.role`; the runtime does not need it after provider-side role conversion. | Renamed to `_role` with `#[serde(rename = "role")]`; no allow required. |
| `cc-api` Vertex region wrapper | `resolve_region_for_model` | Public API / runtime helper | Runtime uses `resolve_region_for_model_with_default`; tests cover per-model overrides and default fallback. Wrapper remains useful for environment-default callers. | Removed allow. |
| `cc-engine` unused imports | `types/config.rs`, `skill_tool.rs` | Truly unused imports | `cargo check -p cc-engine --all-targets` succeeds after trimming imports. | Removed unused imports and all `unused_imports` allows in `cc-engine`. |
| `cc-engine` input overrides | `ProcessedInput.allowed_tools`, `ProcessedInput.model` | Missing implementation | Slash-command override fields are still always `None`; tests now assert the current behavior. They remain a public parsed-input contract until command override wiring is decided. | Removed field allows; no warning because this is public API. |
| `cc-engine` config types | File-level `#![allow(dead_code)]` in `types/config.rs` | Public API / Missing implementation | `QueryParams`, `QuerySource`, `TaskBudget`, and `QueryEngineConfig` are used by lifecycle, query loop, daemon, agents, and root startup; unused fields represent public config parity rather than private dead code. | Removed broad file-level allow. |
| `cc-engine` lifecycle accessors | Sleep, abort reason, permission denials, turn count, tool registry, discovered skills, nested memory paths | Stale allow / Public API | Existing lifecycle tests cover abort, permission denials, turn count, discovered skills, and nested memory paths; added sleep-control and `set_tools` tests. | Removed accessor allows. |
| `cc-engine` orphaned permission flag | `has_handled_orphaned_permission` | Missing implementation | Field is not wired yet, but represents upstream parity for orphaned permission recovery. | Kept a single narrow `dead_code` allow with an inline comment. |
| `cc-engine` abort variants | `AbortReason::{MaxBudget, MaxTurns, ApiError}` | Public API / Missing implementation | `MaxBudget` is set by lifecycle budget enforcement; `MaxTurns` and `ApiError` are public abort taxonomy reserved for query-loop parity. | Removed variant allows. |
| `cc-engine` query response fields | `ModelResponse.stream_events`, `ModelResponse.usage` | Public API | Lifecycle deps populates both fields; query tests and lifecycle usage paths consume model usage through assistant messages. | Removed field allows. |
| `cc-engine` helpers | `has_tool_use`, `build_skills_listing`, `payload_from_value` | Stale allow | Stop-hook tests cover `has_tool_use`; skill prompt/test path calls `build_skills_listing`; added `payload_from_value` parse test. | Removed allows. |

Current first-batch status:

| Crate | Remaining `allow(dead_code)` / `allow(unused_imports)` | Evidence |
| --- | ---: | --- |
| `cc-api` | 0 | `rg` over `crates/cc-api/src` finds no `dead_code` or `unused_imports` allowances. |
| `cc-engine` | 1 `dead_code`, 0 `unused_imports` | Only `QueryEngine::has_handled_orphaned_permission` remains, documented as missing orphaned-permission recovery wiring. |

Dependency-chain crates were not expanded in this batch. The remaining `cc-*`, `voice`, and `worktree` rows below stay as inventory for the next pass.

## `allow(unused_imports)` Inventory

| Type | File | Lines | Context |
| --- | --- | --- | --- |
| standalone | `crates/voice/src/mod.rs` | 38, 40, 42, 44, 46 | `voice` crate top-level re-exports: audio, controller, feasibility, language, stt |
| standalone | `crates/cc-bootstrap/src/lib.rs` | 36, 38, 41 | Top-level re-exports: `ModelSetting`, `Signal`, `DurationTracker` |
| standalone | `crates/cc-teams/src/types.rs` | 122 | Split-crate compatibility re-export: `TeamContext`, `TeammateInfo` |
| standalone | `crates/cc-session/src/session_export/mod.rs` | 30, 32 | `session_export` re-exports marked for `commands/session_export.rs` |
| standalone | `crates/cc-observability/src/lib.rs` | 35 | Observability crate top-level event type re-export |
| standalone | `crates/cc-query/src/lib.rs` | 13, 15, 17 | Query crate top-level API re-exports |
| standalone | `crates/claude-code-rs/src/ui/permissions/dialog_overlay.rs` | 4 | Rust TUI permissions overlay imports `Modifier`, `Style` |

## `allow(dead_code)` Inventory

| File | Hits | Type | Lines | Context |
| --- | ---: | --- | --- | --- |
| `crates/cc-bootstrap/src/lib.rs` | 6 | standalone | 21, 23, 25, 27, 29, 31 | Module re-exports |
| `crates/cc-browser/src/common.rs` | 1 | standalone | 125 | `BrowserConfig` |
| `crates/cc-browser/src/setup.rs` | 1 | standalone | 45 | `profile` field |
| `crates/cc-browser/src/state.rs` | 2 | standalone | 35, 38 | Browser connection states |
| `crates/cc-browser/src/transport.rs` | 5 | 4 standalone, 1 conditional | 69, 94, 132, 148, 156 | Socket path / directory / security helpers; line 69 is `#[cfg_attr(not(unix), allow(dead_code))]` |
| `crates/cc-commands/src/diff.rs` | 1 | standalone | 117 | `get_status_summary` |
| `crates/cc-commands/src/lib.rs` | 1 | standalone | 494 | `None` enum variant |
| `crates/cc-config/src/features.rs` | 1 | standalone | 26 | `KairosGithubWebhooks` |
| `crates/cc-config/src/settings.rs` | 12 | standalone | 70, 700, 707, 733, 735, 740, 835, 968, 974, 983, 994, 1005 | Config ranking, types, fields, helpers |
| `crates/cc-daemon/src/channels.rs` | 1 | standalone | 3 | File/module scope |
| `crates/cc-daemon/src/gateway_bridge.rs` | 1 | standalone | 7 | File/module scope |
| `crates/cc-daemon/src/notification.rs` | 1 | standalone | 3 | File/module scope |
| `crates/cc-daemon/src/routes.rs` | 1 | standalone | 8 | File/module scope |
| `crates/cc-daemon/src/state.rs` | 1 | standalone | 6 | File/module scope |
| `crates/cc-daemon/src/webhook.rs` | 1 | standalone | 3 | File/module scope |
| `crates/cc-engine/src/lifecycle/mod.rs` | 1 | standalone | 125 | `has_handled_orphaned_permission`; missing orphaned-permission recovery wiring |
| `crates/cc-ipc-protocol/src/subsystem_events.rs` | 1 | standalone | 12 | File/module scope; pre-defined IPC extension types |
| `crates/cc-ipc-protocol/src/subsystem_types.rs` | 1 | standalone | 13 | File/module scope; pre-defined IPC extension types |
| `crates/cc-keybindings/src/action.rs` | 1 | standalone | 10 | File/module scope |
| `crates/cc-keybindings/src/config.rs` | 1 | standalone | 23 | File/module scope |
| `crates/cc-keybindings/src/context.rs` | 1 | standalone | 9 | File/module scope |
| `crates/cc-keybindings/src/defaults.rs` | 1 | standalone | 7 | File/module scope |
| `crates/cc-keybindings/src/keystroke.rs` | 1 | standalone | 15 | File/module scope |
| `crates/cc-keybindings/src/registry.rs` | 1 | standalone | 14 | File/module scope |
| `crates/cc-lsp-service/src/client.rs` | 1 | standalone | 597 | `shutdown` |
| `crates/cc-lsp-service/src/mod.rs` | 3 | standalone | 351, 376, 404 | `language_for_extension`, `ServerState`, `set_event_sender` |
| `crates/cc-lsp-service/src/tool.rs` | 1 | standalone | 67 | `method` |
| `crates/cc-mcp/src/channel.rs` | 3 | standalone | 11, 20, 28 | Channel support and notification parsing |
| `crates/cc-mcp/src/client.rs` | 1 | standalone | 651 | `read_resource` |
| `crates/cc-mcp/src/discovery.rs` | 1 | standalone | 27 | `Ide` variant |
| `crates/cc-mcp/src/lib.rs` | 2 | standalone | 130, 369 | Error variant and `ReadResourceResult` |
| `crates/cc-mcp/src/manager.rs` | 3 | standalone | 207, 216, 224 | Resource/client/disconnect helpers |
| `crates/cc-permissions/src/decision.rs` | 3 | standalone | 166, 203, 224 | Decision reason, `is_noop`, `record_denial` |
| `crates/cc-permissions/src/rules.rs` | 4 | standalone | 26, 34, 100, 170 | Permission result/check/match helpers |
| `crates/cc-plugins/src/loader.rs` | 1 | standalone | 68 | `plugins` field |
| `crates/cc-plugins/src/mod.rs` | 2 | standalone | 73, 82 | Marketplace path helpers |
| `crates/cc-query/src/deps.rs` | 2 | standalone | 16, 18 | `stream_events`, `usage` fields |
| `crates/cc-query/src/stop_hooks.rs` | 1 | standalone | 35 | `has_tool_use` |
| `crates/cc-safety/src/classifier.rs` | 1 | standalone | 3 | File/module scope |
| `crates/cc-sandbox/src/availability.rs` | 1 | standalone | 12 | File/module scope |
| `crates/cc-sandbox/src/errors.rs` | 1 | standalone | 7 | File/module scope |
| `crates/cc-sandbox/src/filesystem.rs` | 1 | standalone | 12 | File/module scope |
| `crates/cc-sandbox/src/runner.rs` | 1 | standalone | 36 | `mechanism` field |
| `crates/cc-teams/src/backend.rs` | 3 | standalone | 67, 104, 112 | Teammate/pane backend types |
| `crates/cc-teams/src/coordinator.rs` | 1 | standalone | 1 | File/module scope |
| `crates/cc-teams/src/in_process.rs` | 2 | standalone | 191, 198 | Task registry helpers |
| `crates/cc-teams/src/lib.rs` | 1 | standalone | 28 | `is_agent_teams_enabled` |
| `crates/cc-types/src/agent_channel.rs` | 1 | standalone | 7 | File/module scope |
| `crates/cc-types/src/agent_events.rs` | 1 | standalone | 10 | File/module scope |
| `crates/cc-types/src/agent_types.rs` | 1 | standalone | 11 | File/module scope |
| `crates/cc-types/src/message.rs` | 1 | standalone | 1 | File/module scope |
| `crates/cc-types/src/state.rs` | 6 | standalone | 103, 119, 121, 123, 125, 127 | State enum and usage fields |
| `crates/cc-types/src/transitions.rs` | 1 | standalone | 1 | File/module scope |
| `crates/cc-utils/src/shell.rs` | 1 | standalone | 29 | `display_name` |
| `crates/claude-code-rs/src/app_subsystem_handlers.rs` | 1 | standalone | 404 | `handle_mcp_command` |
| `crates/claude-code-rs/src/ui/agents/mod.rs` | 14 | standalone | 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29 | UI module re-exports |
| `crates/claude-code-rs/src/ui/agents/new_agent_creation/mod.rs` | 2 | standalone | 5, 7 | UI module re-exports |
| `crates/claude-code-rs/src/ui/agents/new_agent_creation/wizard_steps/mod.rs` | 12 | standalone | 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25 | UI module re-exports |
| `crates/claude-code-rs/src/ui/app.rs` | 8 | standalone | 1, 3, 5, 7, 9, 11, 13, 16 | UI module re-exports |
| `crates/claude-code-rs/src/ui/diff.rs` | 3 | standalone | 3, 5, 7 | UI module re-exports |
| `crates/claude-code-rs/src/ui/hooks/mod.rs` | 6 | standalone | 3, 5, 7, 9, 11, 13 | UI module re-exports |
| `crates/claude-code-rs/src/ui/lsp_recommendation/mod.rs` | 1 | standalone | 3 | UI module re-export |
| `crates/claude-code-rs/src/ui/mcp/mod.rs` | 18 | standalone | 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37 | UI module re-exports |
| `crates/claude-code-rs/src/ui/mcp/utils/mod.rs` | 1 | standalone | 3 | UI module re-export |
| `crates/claude-code-rs/src/ui/memory/mod.rs` | 2 | standalone | 3, 5 | UI module re-exports |
| `crates/claude-code-rs/src/ui/messages.rs` | 35 | standalone | 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54, 57, 60, 63, 66, 69, 72, 75, 78, 81, 84, 87, 90, 93, 96, 99, 102, 105 | UI message module re-exports |
| `crates/claude-code-rs/src/ui/messages/user_tool_result_message/mod.rs` | 8 | standalone | 5, 7, 9, 11, 13, 15, 17, 19 | UI module re-exports |
| `crates/claude-code-rs/src/ui/mod.rs` | 59 | standalone | 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 36, 39, 42, 45, 48, 53, 56, 59, 62, 67, 70, 72, 75, 78, 82, 87, 95, 98, 101, 106, 109, 112, 115, 118, 124, 127, 130, 133, 136, 139, 142, 145, 148, 151, 159, 162, 165, 170, 173, 176, 181, 186, 189, 192, 195, 198, 203 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions.rs` | 32 | standalone | 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 39, 41, 43, 45, 47, 49, 51, 53, 55, 57, 59, 61, 63, 65 | UI permission module re-exports |
| `crates/claude-code-rs/src/ui/permissions/ask_user_question_permission_request/mod.rs` | 7 | standalone | 5, 7, 9, 11, 13, 15, 17 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/bash_permission_request/mod.rs` | 2 | standalone | 5, 7 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/computer_use_approval/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/enter_plan_mode_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/exit_plan_mode_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/file_edit_permission_request/mod.rs` | 2 | standalone | 5, 7 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/file_permission_dialog/mod.rs` | 5 | standalone | 5, 7, 9, 11, 13 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/file_write_permission_request/mod.rs` | 2 | standalone | 5, 7 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/filesystem_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/monitor_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/notebook_edit_permission_request/mod.rs` | 2 | standalone | 5, 7 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/power_shell_permission_request/mod.rs` | 2 | standalone | 5, 7 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/review_artifact_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/rules/mod.rs` | 8 | standalone | 3, 5, 7, 9, 11, 13, 15, 17 | UI module re-exports |
| `crates/claude-code-rs/src/ui/permissions/sed_edit_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/skill_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/permissions/web_fetch_permission_request/mod.rs` | 1 | standalone | 5 | UI module re-export |
| `crates/claude-code-rs/src/ui/rendering/virtual_scroll.rs` | 4 | standalone | 138, 152, 178, 230 | Virtual scroll accessors |
| `crates/claude-code-rs/src/ui/skills/mod.rs` | 1 | standalone | 3 | UI module re-export |
| `crates/claude-code-rs/src/ui/tasks/mod.rs` | 14 | standalone | 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29 | UI task module re-exports |
| `crates/claude-code-rs/src/ui/teams/mod.rs` | 2 | standalone | 3, 5 | UI module re-exports |
| `crates/claude-code-rs/tests/e2e_browser_mcp.rs` | 1 | standalone | 29 | `test_workspace` module |
| `crates/claude-code-rs/tests/e2e_chrome_subsystem.rs` | 1 | standalone | 20 | `test_workspace` module |
| `crates/voice/src/mod.rs` | 1 | standalone | 30 | File/module scope |
| `crates/worktree/src/tool.rs` | 1 | standalone | 360 | `discard_changes` field |

## Cleanup Buckets

These buckets are only triage aids:

| Bucket | Relevant rows | Suggested next action |
| --- | --- | --- |
| Root UI re-export shims | `crates/claude-code-rs/src/ui/**` | Decide whether root-owned UI module re-export allowances are intentional residuals or can be removed after the `cc-ui` path-shim closeout. |
| Split-crate compatibility re-exports | `cc-bootstrap`, `cc-query`, `cc-session`, `cc-teams`, `voice` | Replace with used public API, narrower `pub use`, or comments that explain compatibility boundaries. |
| Predefined protocol/types | `cc-ipc-protocol`, `cc-types`, `cc-keybindings` | Keep only if the type is intentionally reserved for near-term protocol/API parity; otherwise remove or test. |
| Runtime helper methods and fields | daemon, MCP, permissions, LSP, browser, sandbox | Prefer targeted tests or real call sites over `allow(dead_code)` before the release gate. |
| Conditional platform API | `cc-browser/src/transport.rs:69` | Keep or replace with platform-specific module boundaries; it is the only conditional `dead_code` hit. |
