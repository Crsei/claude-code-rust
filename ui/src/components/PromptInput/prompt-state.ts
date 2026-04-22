import { formatWorkedDuration } from './keys.js'
import { isPasteInput } from './utils.js'

/**
 * Pure helpers for composing the composer's visible state. The
 * keyboard handler in `InputPrompt.tsx` already owns the text buffer;
 * these helpers exist so we can unit-test the derived rendering values
 * (cursor split, paste-compact predicate, busy-status tag) without
 * mounting the component.
 *
 * Lite-native siblings of the presentational math baked into the
 * sample tree's `BaseTextInput`
 * (`ui/examples/upstream-patterns/src/components/BaseTextInput.tsx`).
 */

/** How many characters of pasted input trigger the compacted
 *  `pasted text Nkb` badge. Kept in one place so the predicate used by
 *  the display layer (`shouldRenderPasteCompact`) matches the detection
 *  threshold in `utils.ts` without drifting. */
export const PASTE_COMPACT_CHARS = 200

export interface BufferSplit {
  before: string
  cursorChar: string
  after: string
}

/**
 * Split the buffer around the cursor for the three-segment render
 * (`before`, `cursorChar`, `after`). When the cursor sits past the last
 * character we surface a single space so the inverted cursor square
 * still has a glyph to paint.
 */
export function splitBufferAtCursor(text: string, cursorPos: number): BufferSplit {
  const clamped = Math.max(0, Math.min(cursorPos, text.length))
  return {
    before: text.slice(0, clamped),
    cursorChar: clamped < text.length ? text[clamped]! : ' ',
    after: clamped < text.length ? text.slice(clamped + 1) : '',
  }
}

/**
 * When to swap the three-segment buffer for the compact
 * `pasted text Nkb` badge: the paste marker has been set AND the
 * current buffer is long enough to actually look like a paste (so a
 * tiny post-paste deletion doesn't leave the badge behind).
 */
export function shouldRenderPasteCompact(
  isPasted: boolean,
  textLength: number,
): boolean {
  return isPasted && textLength >= PASTE_COMPACT_CHARS
}

/** Consistency check for callers that want the same detection
 *  threshold as `shouldRenderPasteCompact`. Re-exports
 *  `isPasteInput(textLength)` so they don't need to import both modules
 *  to stay in sync. */
export { isPasteInput }

export interface BusyStatus {
  /** `'reasoning'` while the backend is streaming the final answer,
   *  `'thinking'` while it is preparing tool calls, empty otherwise. */
  modeTag: string
  /** Full status label including the elapsed duration — for example
   *  `"reasoning 3s"`. Empty when the composer is idle. */
  workedTag: string
}

/**
 * Build the busy-status tag shown next to the composer cursor. Centralizes
 * the reasoning/thinking wording so the inline status hint and the border
 * title stay in lockstep.
 */
export function buildBusyStatus(params: {
  isStreaming: boolean
  isWaiting: boolean
  isBusy: boolean
  lastWorkedMs: number
  workedMs: number
}): BusyStatus {
  const { isStreaming, isWaiting, isBusy, lastWorkedMs, workedMs } = params
  const modeTag = isStreaming ? 'reasoning' : isWaiting ? 'thinking' : ''
  const showWorked = isBusy || lastWorkedMs > 0
  if (!showWorked) {
    return { modeTag, workedTag: '' }
  }
  const duration = formatWorkedDuration(workedMs)
  const workedTag = modeTag ? `${modeTag} ${duration}` : duration
  return { modeTag, workedTag }
}

/**
 * Compose the external status string surfaced through the optional
 * `onStatusChange` callback. Returns an empty string in transcript
 * mode so the consumer (`App.tsx`) doesn't leak a stale busy tag into
 * the composer title during a view-mode switch.
 */
export function deriveExternalStatus(
  viewMode: 'prompt' | 'transcript',
  workedTag: string,
): string {
  if (viewMode !== 'prompt' || !workedTag) return ''
  return `* ${workedTag}`
}
