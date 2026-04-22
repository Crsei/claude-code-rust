/**
 * Barrel for the Lite-native composer/prompt-input decomposition
 * (Issue 06).
 *
 * Each submodule owns one concern lifted out of the previously
 * monolithic `InputPrompt.tsx`:
 * - `ComposerBuffer` — visible buffer row (before/cursor/after, paste
 *   compact, transcript-mode readonly branch).
 * - `QueuedSubmissions` — queued-submission preview row.
 * - `ModeIndicator` — inline reasoning/thinking status label.
 * - `SlashCommandHints` — slash-command autocomplete + sub-mode
 *   option selector.
 * - `useComposerSubmit` — submit / queue / slash-command dispatch
 *   hook.
 * - `hooks.ts` — composer state, busy timer, paste handler, history
 *   navigator (existing hooks, moved here from the flat
 *   `input-prompt-hooks.ts`).
 * - `keys.ts` — key-event normalization helpers.
 * - `utils.ts` — paste detection, placeholder, queued summary, cursor
 *   insertion.
 * - `prompt-state.ts` — pure rendering math (cursor split, paste
 *   compact predicate, busy status tag, external status).
 *
 * Keyboard handling still lives in `InputPrompt.tsx`; the submodules
 * are consumed from that single orchestrator.
 */
export { ComposerBuffer } from './ComposerBuffer.js'
export {
  useBusyTimer,
  useComposerState,
  useInputHistoryNav,
  usePasteHandler,
  type ComposerState,
} from './hooks.js'
export {
  extractInput,
  formatWorkedDuration,
  toShortcutKey,
  type ShortcutKey,
} from './keys.js'
export { ModeIndicator } from './ModeIndicator.js'
export {
  PASTE_COMPACT_CHARS,
  buildBusyStatus,
  deriveExternalStatus,
  shouldRenderPasteCompact,
  splitBufferAtCursor,
  type BufferSplit,
  type BusyStatus,
} from './prompt-state.js'
export { QueuedSubmissions } from './QueuedSubmissions.js'
export { SlashCommandHints } from './SlashCommandHints.js'
export { useComposerSubmit } from './useComposerSubmit.js'
export {
  formatPasteSize,
  insertAtCursor,
  isPasteInput,
  isPlainTextInput,
  promptPlaceholder,
  summarizeQueuedSubmissions,
} from './utils.js'
