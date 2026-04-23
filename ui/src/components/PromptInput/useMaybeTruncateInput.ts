import { useEffect, useRef } from 'react'

/**
 * Guard against pasted / typed inputs that are large enough to lag the
 * OpenTUI renderer. Mirrors upstream
 * `ui/examples/upstream-patterns/src/components/PromptInput/useMaybeTruncateInput.ts`
 * at a reduced scope: we only stash a placeholder marker in place of
 * the middle slice — the full text is kept in a ref so the caller can
 * reconstitute it on submit without round-tripping through the store.
 *
 * The truncation is idempotent per buffer instance: once we've applied
 * it, we don't re-truncate the same input until the buffer is cleared
 * (which happens on submit / cancel). That avoids feedback loops when
 * the parent effect replays the current buffer back through
 * `setText`.
 */

const TRUNCATION_THRESHOLD = 10_000
const PREVIEW_LENGTH = 1_000

export interface TruncatedPasteRef {
  id: number
  content: string
}

export interface UseMaybeTruncateInputParams {
  text: string
  setText: (value: string) => void
  setCursorPos: (value: number) => void
}

export interface UseMaybeTruncateInputResult {
  /**
   * Pop the stashed full-size text on submit. Returns the supplied
   * `currentText` unchanged if no truncation happened during this
   * buffer's lifetime.
   */
  rehydrate: (currentText: string) => string
  /** Read-only access to the most recent stash (null when nothing was
   *  truncated during this buffer's lifetime). */
  getStash: () => TruncatedPasteRef | null
}

export function useMaybeTruncateInput({
  text,
  setText,
  setCursorPos,
}: UseMaybeTruncateInputParams): UseMaybeTruncateInputResult {
  // Stashed middle slice of the current oversized buffer — null unless
  // we've already truncated this particular input.
  const stashRef = useRef<TruncatedPasteRef | null>(null)
  // Next unique id for the placeholder label. Monotonic across the life
  // of the component so sequential truncations don't collide visually.
  const nextIdRef = useRef(1)

  useEffect(() => {
    // Drop the stash as soon as the buffer empties so the next paste
    // starts from a clean slate.
    if (text.length === 0) {
      stashRef.current = null
      return
    }

    if (stashRef.current !== null) {
      // Already truncated this buffer — don't re-run the effect even if
      // the caller replays text into the buffer (React dev-mode effect
      // replays, undo/redo, etc.).
      return
    }

    if (text.length <= TRUNCATION_THRESHOLD) {
      return
    }

    const half = Math.floor(PREVIEW_LENGTH / 2)
    const startText = text.slice(0, half)
    const endText = text.slice(-half)
    const middle = text.slice(half, text.length - half)
    const id = nextIdRef.current
    nextIdRef.current += 1
    const lineCount = middle.split('\n').length
    const placeholder = `[...Truncated #${id} +${lineCount} lines...]`
    const truncated = startText + placeholder + endText
    stashRef.current = { id, content: middle }
    setText(truncated)
    setCursorPos(truncated.length)
  }, [text, setText, setCursorPos])

  const rehydrate = (currentText: string): string => {
    const stash = stashRef.current
    if (!stash) return currentText
    const marker = `[...Truncated #${stash.id} `
    const start = currentText.indexOf(marker)
    if (start === -1) {
      // Placeholder was edited out by the user — honor their edit.
      return currentText
    }
    const end = currentText.indexOf(']', start)
    if (end === -1) return currentText
    return currentText.slice(0, start) + stash.content + currentText.slice(end + 1)
  }

  return {
    rehydrate,
    getStash: () => stashRef.current,
  }
}

/** Exported for tests. */
export const TRUNCATION_THRESHOLD_CHARS = TRUNCATION_THRESHOLD
