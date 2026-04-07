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
