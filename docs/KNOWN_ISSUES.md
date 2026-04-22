# Known Issues — Terminal UI / Frontend

This document tracks reported UI/UX issues in the terminal frontends and headless UI bridge.
Each issue includes a description, reproduction steps, and current status.

---

## 1. Terminal resize does not reflow content

**Status**: Fixed for Rust TUI (2026-04-19, issue #12). Still open for the TS/OpenTUI frontend.

**Description**: When the terminal window is resized (e.g., dragging the window border to shrink/expand), the rendered content does not adapt to the new dimensions. The user must use `Ctrl + -` / `Ctrl + +` to zoom out/in to see the full content. This affects all components: message list, input prompt, tool output blocks, etc.

**Expected behavior**: Content should dynamically reflow to fit the current terminal width/height on resize, similar to how standard terminal applications handle `SIGWINCH`.

**Reproduction**:
1. Start the UI with `run.sh`
2. Have a conversation with some long assistant responses
3. Drag the terminal window to a narrower width
4. Observe that text is clipped or overflows rather than reflowing

**Fix (Rust side)**:
- `src/ui/tui.rs` already forwards `Event::Resize` to `App::mark_dirty`
- `src/ui/virtual_scroll.rs` now has explicit regression tests confirming
  a width change invalidates the height cache, re-measures every message,
  and produces a new prefix-sum offset table
- `src/ui/welcome.rs` gained `welcome_height_for(width)` so narrow
  terminals don't reserve 16 rows when they only need 8 / 12

**Still open (TS/OpenTUI side)**:
The OpenTUI rendering layer may not be re-measuring
`process.stdout.columns` / `process.stdout.rows` on resize events, or
components are not subscribing to terminal dimension changes.

**Related files**:
- `ui/src/ipc/protocol.ts` — `Resize` message type
- `ui/src/components/MessageList.tsx` — uses `process.stdout.columns` at render time
- `ui/src/components/App.tsx` — top-level layout

---

*Add new issues below this line.*

## 2. Composer lacked a frame and busy indicator could ghost in the footer

**Status**: Fixed (2026-04-08)

**Description**: The bottom composer was rendered as raw prompt text without a visible input frame. During the transition from busy state back to idle, the standalone `Thinking...` / `Reasoning...` indicator could remain as a stale line in the lower-left corner even after the assistant response had finished rendering.

**Expected behavior**: The composer should always render inside a stable bordered container, and the busy indicator should be repainted inside that same container so footer state changes do not leave stale rows behind.

**Fix**:
1. Rebuilt the composer with a bordered terminal `Box`
2. Moved idle and busy rendering into the same composer component
3. Cleared frontend streaming state more aggressively on `stream_end` and `error`

**Related files**:
- `ui/src/components/InputPrompt.tsx`
- `ui/src/components/App.tsx`
- `ui/src/store/app-store.tsx`

## 3. Message area had no visible scroll affordance and was hard to navigate

**Status**: Fixed (2026-04-08)

**Description**: In long conversations, the message area could exceed the viewport but users had no clear in-app scroll affordance. They often had to zoom out (`Ctrl + -`) to view complete content.

**Expected behavior**: The conversation should remain readable at normal zoom, with explicit scrolling controls and a visible state indicator.

**Fix**:
1. Added message list scrolling with mouse wheel.
2. Added keyboard scrolling: `PageUp`, `PageDown`, `Ctrl+↑`, `Ctrl+↓`, `Ctrl+Home`, `Ctrl+End`.
3. Added a footer hint in the message area showing scroll position and whether more content exists above/below.
4. Routed wheel behavior by click focus: click message area => wheel scrolls conversation; click composer => wheel navigates input history.
5. Rebuilt the composer UI using an explicit bordered input row + status row layout to ensure the input box is always visible.
6. Reduced long-response rendering pressure: removed scroll metric polling and switched streaming-phase rendering to plain text; final assistant message still renders via Markdown.
7. Enforced responsive wrapping for long single-line output by constraining message/tool blocks to `width: 100%`, enabling explicit text wrapping, and subscribing message virtualization to terminal `resize` updates.

**Related files**:
- `ui/src/components/MessageList.tsx`

## 4. Welcome screen Tips text truncated in narrow terminals (< 80 cols)

**Status**: Open  
**Discovered**: 2026-04-09 via PTY screenshot testing

**Description**: At 60 columns, the Tips section text wraps mid-sentence with hard line breaks:
- "Type a message and press Enter" wraps to "to send" on a new line
- "Start with / for slash" wraps to "commands" on the next line  
- "/model to switch models" disappears entirely (no space left)

**Expected behavior**: Tips text should gracefully truncate or abbreviate when terminal width is insufficient, rather than creating orphaned word fragments. Alternatively, the two-column layout (ASCII logo + Tips) should collapse to single-column below a width threshold.

**Reproduction**:
1. `cargo test --test pty_ui screenshot_narrow -- --nocapture`
2. Open generated `logs/YYYYMMDDHHMM/screenshot_narrow.html` in browser
3. Observe Tips text wrapping and missing entries

**Related files**:
- `src/ui/welcome.rs` — welcome screen layout and text rendering

## 5. ASCII art logo renders as fragmented blocks in narrow terminals

**Status**: Open  
**Discovered**: 2026-04-09 via PTY screenshot testing

**Description**: The "CC" ASCII art logo (using Unicode block characters `██████╗`) renders as disconnected purple blocks at both 120-col and 60-col widths. The block elements `█` and box-drawing characters `╗╔║╚╝═` appear with visible gaps between them, making the logo look fragmented rather than forming clean solid letters.

**Expected behavior**: The ASCII art should display as two recognizable "C" letters with connected block strokes, as originally designed.

**Likely cause**: The TUI renders each cell independently. When box-drawing and block characters span multiple cells, some terminals or font configurations introduce sub-pixel gaps between adjacent cells. The ratatui renderer may also be inserting attribute-reset sequences between adjacent same-colored cells, which could cause visual fragmentation.

**Reproduction**:
1. `cargo test --test pty_ui screenshot_welcome -- --nocapture`
2. Open generated `logs/YYYYMMDDHHMM/screenshot_welcome.html` in browser
3. Observe the purple ASCII logo area — blocks are disconnected

**Related files**:
- `src/ui/welcome.rs` — ASCII art logo definition and rendering

## 6. Background agent + worktree isolation 未组合

**Status**: Open (设计限制)
**Discovered**: 2026-04-10

**Description**: Agent 工具的 `run_in_background: true` 和 `isolation: "worktree"` 参数无法同时生效。当两者都指定时，worktree 隔离被忽略，后台代理使用当前工作目录运行，并输出一条 warning 日志。

**原因**: Worktree 创建需要异步 git 操作 (`git worktree add`)，而后台代理需要在 `tokio::spawn` 之前构建好 child config（需要确定 cwd）。在 spawn 闭包内执行异步 worktree 创建会增加错误处理复杂度，且 worktree 清理逻辑（检测变更、删除分支）需要在 spawn 内完成。

**后续计划**: 将 worktree 创建移入 spawn 闭包内部，复用现有 `run_in_worktree` 逻辑。

**Related files**:
- `src/tools/agent.rs` — `call()` 方法的 background spawn 路径

## 7. Background agent 子引擎无 permission_callback

**Status**: Open (设计限制)
**Discovered**: 2026-04-10

**Description**: 通过 `run_in_background: true` 启动的后台代理创建的子 `QueryEngine` 没有设置 `permission_callback`。在 default 权限模式下，子代理执行需要 `Ask` 权限的工具（如 Bash、FileWrite）时会被直接拒绝，而不是提示用户确认。

**影响**: 后台代理在 `auto` 或 `bypass` 权限模式下正常工作；在 `default` 模式下，只有不需要权限确认的只读工具（Glob、Grep、FileRead）可以正常执行。

**后续计划**: 将父引擎的 `permission_callback` 传递给子引擎。需要考虑并发权限请求的 UI 展示问题（多个后台代理同时请求权限）。

**Related files**:
- `src/tools/agent.rs` — background spawn 路径中的 `QueryEngine::new(child_config)`
- `src/engine/lifecycle/mod.rs` — `set_permission_callback()`

## 8. Background agent 无取消机制

**Status**: Open (设计限制)
**Discovered**: 2026-04-10

**Description**: 后台代理通过 `tokio::spawn` 启动后，`JoinHandle` 未被保存。用户中断父查询 (`Ctrl+C`) 或退出应用时，后台代理会继续运行直到完成（或 tokio runtime 关闭）。对于长时间运行的后台代理，这可能导致资源浪费。

**后续计划**: 在 `PendingBackgroundResults` 或新的 `BackgroundAgentManager` 中保存 `JoinHandle`，在 `graceful_shutdown` 或用户 abort 时调用 `handle.abort()`。同时需要将父引擎的 `abort_signal` 传递给子引擎。

**Related files**:
- `src/tools/agent.rs` — `tokio::spawn` 调用
- `src/tools/background_agents.rs` — `PendingBackgroundResults`
- `src/shutdown.rs` — `graceful_shutdown()`
## 9. Tool-call display and shortcut discoverability were too weak

**Status**: Fixed (2026-04-13)

**Description**: The frontend rendered `tool_use` and `tool_result` as unrelated top-level blocks, so replayed conversations lost the original activity structure and busy sessions became hard to scan. Shortcut hints were also scattered across components, which made transcript mode and redraw or browse controls hard to discover.

**Expected behavior**: Tool calls should render as a paired activity timeline, read/search bursts should collapse in prompt view, transcript mode should expose expanded activity summaries without dumping raw output, and shortcut labels should come from one shared registry.

**Fix**:
1. Preserved raw `content_blocks` in `conversation_replaced` so replayed history can rebuild tool activity timelines.
2. Split frontend state into raw messages plus derived render items, then grouped `Read` / `Glob` / `Grep` calls in prompt view while keeping transcript view expanded and read-only.
3. Replaced separate tool use/result blocks with unified `ToolActivity` and `ToolGroup` renderers.
4. Centralized fixed keyboard bindings for transcript toggle, redraw, Vim toggle, scrolling, and command-completion hints.

**Related files**:
- `src/ipc/protocol.rs`
- `src/ipc/headless.rs`
- `ui/src/ipc/protocol.ts`
- `ui/src/store/message-model.ts`
- `ui/src/store/app-store.tsx`
- `ui/src/components/App.tsx`
- `ui/src/components/MessageList.tsx`
- `ui/src/components/InputPrompt.tsx`

## 10. Busy prompt could not queue follow-up steering messages

**Status**: Fixed (2026-04-14)

**Description**: While Claude was mid-response or still executing a task, the prompt allowed drafting text but blocked `Enter`, so users could not queue a second follow-up message the way `claude-code-bun` supports "steering while it works".

**Expected behavior**: When the current turn is busy, submitting a normal prompt should enqueue it client-side and automatically send it after the current turn finishes, without forcing an interrupt.

**Fix**:
1. Added a frontend FIFO queue for follow-up prompt submissions.
2. Changed busy `Enter` handling so normal prompts are queued instead of dropped.
3. Added queued-preview rendering in the composer and a queued-count hint in the input title.

**Related files**:
- `ui/src/store/app-store.tsx`
- `ui/src/components/App.tsx`
- `ui/src/components/InputPrompt.tsx`

## 11. Prompt-mode tool groups hid the actual commands and felt too opaque

**Status**: Fixed (2026-04-14)

**Description**: Prompt mode collapsed contiguous `Read` / `Glob` / `Grep` activity into a single `[GROUP] Glob 5, Read 10` line plus one latest path. That made it hard to see what the agent was currently reading or searching, and long tool bursts gave no obvious hint that `Ctrl+O` could reveal a fuller per-tool breakdown in transcript mode.

**Expected behavior**: Prompt mode should keep grouped tool bursts compact, but still preview the actual commands or paths being used, truncate long previews, and show an explicit expand hint. Transcript mode should then expose each tool call with its full input detail instead of the shortened prompt summary.

**Fix**:
1. Replaced the opaque `[GROUP]` label with human-readable summaries like `Read 3 files, Glob 1 pattern`.
2. Added per-group preview lines showing the most recent tool commands or paths, with truncation and `+N more tool uses` folding.
3. Added an inline `Ctrl+O` expand hint on grouped activity blocks.
4. Switched transcript-mode tool activity input rendering to use full tool input details rather than the compact summary string.

**Related files**:
- `ui/src/store/message-model.ts`
- `ui/src/store/message-model.test.ts`
- `ui/src/components/ToolGroup.tsx`
- `ui/src/components/ToolActivity.tsx`

## 12. Composer stopped accepting pasted text

**Status**: Fixed (2026-04-14)

**Description**: The prompt composer only handled keyboard events and treated input as single-character text. After bracketed paste mode was enabled in the terminal layer, pasted content arrived as a dedicated paste event or as multi-character text chunks, so the composer silently dropped it instead of inserting it.

**Expected behavior**: Pasting into the composer should insert the full pasted text at the current cursor position, preserve multi-line content, and still keep the compact pasted-size rendering for large payloads.

**Fix**:
1. Subscribed `InputPrompt` to the renderer's internal paste event stream.
2. Inserted pasted bytes directly into the composer at the current cursor position.
3. Taught keyboard text extraction to accept plain multi-character text chunks, not just single characters.
4. Moved paste-related helper logic into a pure utility module and added regression tests for plain-text detection and cursor insertion.

**Related files**:
- `ui/src/components/InputPrompt.tsx`
- `ui/src/components/input-prompt-utils.ts`
- `ui/src/components/__tests__/paste-display.test.ts`

## 13. IME multi-character input only kept the last committed character

**Status**: Fixed (2026-04-14)

**Description**: After the composer gained multi-character plain-text insertion, the new insertion path read `text` and `cursorPos` from the render closure instead of the live refs. When an IME committed multiple characters in one burst, each insert reused stale state and overwrote the previous insert, so only the last character remained visible.

**Expected behavior**: CJK input methods should preserve the full committed text, even when the terminal delivers multiple committed characters in one event burst.

**Fix**:
1. Switched composer insertion and undo capture to use the live `textRef` / `cursorRef` values.
2. Updated the multi-character insertion path so burst inserts accumulate correctly before the next React render commits.

**Related files**:
- `ui/src/components/InputPrompt.tsx`

## 14. Conversation rows lacked visual separation and file references blended into code

**Status**: Fixed (2026-04-14)

**Description**: User and assistant messages were rendered with nearly the same visual weight, while redundant `You` / `Assistant` labels consumed vertical space without adding much signal. File paths and code snippets also shared the same plain-text treatment, which made transcript scanning slower.

**Expected behavior**: User messages should be visually separated with a darker row treatment, speaker labels should be removed, and file references should use a distinct highlight color from inline code and fenced code blocks.

**Fix**:
1. Removed the `You` / `Assistant` message labels from chat bubbles.
2. Rendered user messages inside a darker, left-accented container.
3. Updated markdown token formatting so file paths use a cyan highlight while inline code and code blocks use amber styling.
4. Added a markdown formatter regression test covering file-path, inline-code, and fenced-code coloring.

**Related files**:
- `ui/src/components/MessageBubble.tsx`
- `ui/src/theme.ts`
- Historical note: the old `ui/ink-terminal` markdown implementation is no longer part of the maintained frontend path.

## 15. Headless AskUserQuestion left an orphaned tool call and broke the next model request

**Status**: Fixed (2026-04-14)

**Description**: In `--headless` mode, `AskUserQuestion` still tried to read directly from backend stdin. The frontend's next `submit_prompt` therefore started a brand-new query instead of satisfying the pending tool call, leaving an assistant `tool_calls` message without a matching `tool` reply. Azure/OpenAI then rejected the next request with `400 Bad Request` complaining about the missing `tool_call_id`.

**Expected behavior**: When the model asks a question in headless mode, the frontend's next submitted text should be routed back as the answer to that pending `AskUserQuestion` tool call, not treated as a new top-level prompt.

**Fix**:
1. Added an AskUser callback path to `ToolUseContext`.
2. Changed `AskUserQuestion` to prefer the callback over raw stdin reads.
3. Added a headless pending-question bridge so the next `submit_prompt` answers the waiting tool call.
4. Added regression tests for callback-based AskUser handling and pending-question routing.

**Related files**:
- `src/types/tool.rs`
- `src/tools/ask_user.rs`
- `src/ipc/headless.rs`
- `src/engine/lifecycle/mod.rs`
- `src/engine/lifecycle/deps.rs`

## 16. AskUserQuestion tool activity rendered raw JSON instead of the actual question

**Status**: Fixed (2026-04-14)

**Description**: The frontend tool activity view treated `AskUserQuestion` input like any other structured object and fell back to `JSON.stringify`. That left prompt and transcript rows showing raw payloads such as `{"question":"..."}` instead of the actual question text, and the question itself did not stand out visually from ordinary tool arguments.

**Expected behavior**: `AskUserQuestion` should display the question text directly, without JSON framing, and the question should be highlighted so it reads like an in-conversation prompt instead of a low-level tool payload.

**Fix**:
1. Taught the tool-input summarizer to extract `question` fields before falling back to JSON.
2. Rendered `AskUserQuestion` content inside a highlighted callout in both prompt and transcript views.
3. Added a regression test to keep `AskUserQuestion` summaries from regressing back to raw JSON.

**Related files**:
- `ui/src/store/message-model.ts`
- `ui/src/store/message-model.test.ts`
- `ui/src/components/ToolActivity.tsx`
- `ui/src/theme.ts`

---

## 17. Maximizing then restoring the terminal can leave white horizontal artifacts

**Status**: Open (2026-04-22, TS/OpenTUI frontend)

**Description**: On Windows terminals using the OpenTUI frontend, toggling the terminal window between normal size and maximized/fullscreen can leave white horizontal rows across the alternate-screen UI. The app content continues to work, but the stale rows remain visible after the resize finishes.

**Expected behavior**: Entering or leaving a maximized/fullscreen terminal window should trigger a complete repaint of the alternate-screen surface, without leftover rows from the previous frame.

**Evidence so far**:
1. The issue does not reproduce on initial startup, so it is tied to resize handling rather than first paint.
2. OpenTUI's `processResize()` path resizes the renderer and schedules a redraw, but does not explicitly clear the terminal surface in the normal alternate-screen path.
3. The TS frontend now forwards resize events to the backend, but fullscreen/maximize transitions may still need an explicit native clear + immediate rerender to avoid stale rows.

**Related files**:
- `ui/src/components/App.tsx`
- `ui/src/components/resize-sync.ts`
- `ui/node_modules/@opentui/core/index-kgg0v67t.js` — `processResize()`, `clearTerminal()`

---

## Browser MCP rendering path not exercised against a real server

**Status**: Open (2026-04-18)

**Description**: Issues #2 and #3 added the self-hosted Browser MCP integration — detection, system-prompt injection, category-aware permission prompts, screenshot/console/network rendering in the Web UI, and an e2e smoke test for the config→prompt path. None of the runtime rendering has been exercised against a real third-party browser MCP server (e.g. `mcp-chrome`, `@playwright/mcp`, `browser-use-mcp`). The smoke test only validates the config-flag detection path; it does not connect to a live MCP server, drive a browser, or round-trip a screenshot.

Specifically, the following behaviors compile and type-check but have NOT been observed end-to-end:

1. Base64 image bytes flowing from Rust `ToolResultContentInfo::Image` → SSE `user_replay` → Web UI `<img src="data:…;base64,…">`.
2. `BrowserToolResult` screenshot expand/collapse with a real PNG payload and realistic size (we don't know yet whether large screenshots stall the SSE pipe, bloat memory, or look wrong at default viewport dimensions).
3. Structured console/network rendering in `BrowserToolResult` against actual shapes emitted by real servers (our JSON-shape detector is based on docs, not captures — field names like `url`/`method`/`level`/`text` may not match all servers).
4. `tools/call` return values for `navigate` / `read_page` / `click` / `fill` arriving at the `McpToolWrapper` and producing the expected `[category] summary` display preview.
5. Permission dialog wording in the TUI for a real server's tool name (tested via unit tests, but not via a real live `/permission-request` round trip).

**Reproduction (pending)**:
1. Install `mcp-chrome` (or another Browser MCP server) and add it to `.cc-rust/settings.json` per `docs/reference/browser-mcp-config.md`.
2. Start cc-rust with a working API key.
3. Ask the assistant to open a page, take a snapshot, click something, read back the result.
4. Observe: screenshot renders inline in Web UI; console/network results render as structured lists; browser category badge appears on `ToolCallCard`; permission prompts use category-aware wording.

**Next steps when exercised**:
- Capture a real `list_console_messages` / `list_network_requests` JSON payload and add it as a regression fixture to `BrowserToolResult.test.tsx` / Rust `tool_rendering` tests.
- Measure Web UI memory footprint with a screenshot attached to each tool result over a long session (SSE carries full base64 — watch for growth).
- Decide whether to add a size cap / lazy-load path when a screenshot exceeds N KB.

**Related files**:
- `src/browser/` — detection, prompt, permissions, tool_rendering
- `src/ipc/protocol/base.rs` — `ToolResultContentInfo::Image { data }`
- `src/ipc/sdk_mapper.rs` — image forwarding
- `web-ui/src/components/tools/BrowserToolResult.tsx`
- `web-ui/src/components/tools/ToolCallCard.tsx`
- `web-ui/src/lib/browser-tools.ts`
- `ui/src/ipc/protocol.ts` — `ToolResultContentInfo` + `tool_result.content_blocks`
- `tests/e2e_browser_mcp.rs` — current smoke coverage (config flag only)
