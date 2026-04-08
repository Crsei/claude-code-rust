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
