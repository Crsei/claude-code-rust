/**
 * Barrel for the native composer/prompt-input decomposition
 * (Issue 06).
 *
 * Each submodule owns one concern lifted out of the previously
 * monolithic `InputPrompt.tsx`:
 * - `ComposerBuffer` — visible buffer row (before/cursor/after, paste
 *   compact, transcript-mode readonly branch).
 * - `QueuedSubmissions` — numbered queue preview with truncation +
 *   optional selected-row highlight (upstream parity with
 *   `PromptInputQueuedCommands`).
 * - `ModeIndicator` — inline reasoning/thinking status label.
 * - `PromptInputFooter` — compact hint row below the composer (vim
 *   mode, busy tag, queued count, submit/cancel affordance). Trimmed
 *   port of upstream `PromptInputFooter` + `PromptInputFooterLeftSide`.
 * - `SlashCommandHints` — slash-command autocomplete + sub-mode
 *   option selector.
 * - `useComposerSubmit` — submit / queue / slash-command dispatch hook.
 * - `hooks.ts` — composer state, busy timer, paste handler, history
 *   navigator.
 * - `usePromptInputPlaceholder` — rotating placeholder copy + queued
 *   hint when the buffer is empty (upstream parity with
 *   `usePromptInputPlaceholder`).
 * - `useMaybeTruncateInput` — placeholder-swap for pasted content that
 *   would otherwise lag the renderer (upstream parity with
 *   `useMaybeTruncateInput`).
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
export { PromptInputFooter } from './PromptInputFooter.js'
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
  PROMPT_PLACEHOLDER_HINTS,
  usePromptInputPlaceholder,
  type PromptInputPlaceholderOptions,
} from './usePromptInputPlaceholder.js'
export {
  TRUNCATION_THRESHOLD_CHARS,
  useMaybeTruncateInput,
  type TruncatedPasteRef,
  type UseMaybeTruncateInputParams,
  type UseMaybeTruncateInputResult,
} from './useMaybeTruncateInput.js'
export {
  formatPasteSize,
  insertAtCursor,
  isPasteInput,
  isPlainTextInput,
  promptPlaceholder,
  summarizeQueuedSubmissions,
} from './utils.js'
