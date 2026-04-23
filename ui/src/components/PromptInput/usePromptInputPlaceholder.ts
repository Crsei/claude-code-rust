import { useEffect, useState } from 'react'

/**
 * Dynamic placeholder for the empty-buffer composer.
 *
 * Mirrors upstream `usePromptInputPlaceholder` at
 * `ui/examples/upstream-patterns/src/components/PromptInput/usePromptInputPlaceholder.ts`
 * with a Lite-friendly shape:
 *   - surfaces a "press up to edit queued messages" hint when at least
 *     one prompt is queued (the Lite queue cannot be edited in place, but
 *     Up-arrow navigates history which lets the user pull the last queued
 *     text back for editing before it runs);
 *   - otherwise rotates through a small set of onboarding hints so the
 *     composer never looks static.
 *
 * Returns `undefined` when the caller should render no placeholder (for
 * example while the buffer is non-empty or while the backend is busy —
 * the busy hint has its own copy baked into `ComposerBuffer`).
 */

const ONBOARDING_HINTS: string[] = [
  'Type a message or /command...',
  'Try /help to list commands',
  '/status for session info',
  '/cost to see token usage',
  '? for keyboard shortcuts',
]

const ROTATION_INTERVAL_MS = 6000

export interface PromptInputPlaceholderOptions {
  text: string
  isBusy: boolean
  hasQueuedSubmissions: boolean
}

export function usePromptInputPlaceholder({
  text,
  isBusy,
  hasQueuedSubmissions,
}: PromptInputPlaceholderOptions): string | undefined {
  const [hintIndex, setHintIndex] = useState(0)

  useEffect(() => {
    // Only rotate while the composer is idle and empty. Stopping the
    // interval keeps React's scheduler quiet while the user is typing.
    if (text !== '' || isBusy) {
      return
    }
    const id = setInterval(() => {
      setHintIndex(index => (index + 1) % ONBOARDING_HINTS.length)
    }, ROTATION_INTERVAL_MS)
    return () => clearInterval(id)
  }, [text, isBusy])

  if (text !== '') {
    return undefined
  }

  if (isBusy) {
    // Busy placeholder is owned by ComposerBuffer to keep the "Working…"
    // copy consistent with the inline status tag; signal absent here.
    return undefined
  }

  if (hasQueuedSubmissions) {
    return 'Press up to edit queued messages'
  }

  return ONBOARDING_HINTS[hintIndex] ?? ONBOARDING_HINTS[0]
}

/** Exported for unit tests — the list of rotating hints. */
export const PROMPT_PLACEHOLDER_HINTS = ONBOARDING_HINTS
