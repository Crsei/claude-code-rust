import React from 'react'
import { c } from '../../theme.js'
import type { QueuedSubmission } from '../../store/app-state.js'
import { summarizeQueuedSubmissions } from './utils.js'

/**
 * Preview row showing the currently-queued submissions beneath the
 * composer. Rendered only in `prompt` view-mode and when at least one
 * submission is queued.
 *
 * Summarization logic lives in `utils.ts` (`summarizeQueuedSubmissions`)
 * so the truncation + overflow marker behavior is unit-testable.
 */

type Props = {
  submissions: readonly QueuedSubmission[]
}

export function QueuedSubmissions({ submissions }: Props) {
  if (submissions.length === 0) return null
  const preview = summarizeQueuedSubmissions([...submissions])
  return (
    <box paddingLeft={3}>
      <text fg={c.dim}>
        Queued {submissions.length}: {preview}
      </text>
    </box>
  )
}
