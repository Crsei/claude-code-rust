# Known Issues — ink-terminal Frontend (ui/)

This document tracks reported UI/UX issues in the ink-terminal frontend.
Each issue includes a description, reproduction steps, and current status.

---

## 1. Terminal resize does not reflow content

**Status**: Open

**Description**: When the terminal window is resized (e.g., dragging the window border to shrink/expand), the rendered content does not adapt to the new dimensions. The user must use `Ctrl + -` / `Ctrl + +` to zoom out/in to see the full content. This affects all components: message list, input prompt, tool output blocks, etc.

**Expected behavior**: Content should dynamically reflow to fit the current terminal width/height on resize, similar to how standard terminal applications handle `SIGWINCH`.

**Reproduction**:
1. Start the UI with `run.sh`
2. Have a conversation with some long assistant responses
3. Drag the terminal window to a narrower width
4. Observe that text is clipped or overflows rather than reflowing

**Likely cause**: The `Resize` frontend message is sent to the backend but the ink-terminal rendering layer may not be re-measuring `process.stdout.columns` / `process.stdout.rows` on resize events, or components are not subscribing to terminal dimension changes.

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
1. Rebuilt the composer with an `ink-terminal` bordered `Box`
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
5. Rebuilt the composer UI using the `ink-terminal/examples/alternate-screen.tsx` style (explicit bordered input row + status row) to ensure the input box is always visible.
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
