import React from 'react'
import { c } from '../../theme.js'
import type { QueuedSubmission } from '../../store/app-state.js'
import { truncate } from '../../utils.js'

/**
 * Preview block for queued submissions.
 *
 * Upgraded from the original single-line summary to a numbered list
 * (upstream parity with `PromptInputQueuedCommands`) so operators can
 * see *which* prompts will run and in what order. Kept to a capped
 * preview (`MAX_VISIBLE`) to avoid stealing too many rows from the
 * message list; an overflow marker shows how many more are pending.
 *
 * Rendering rules:
 * - Numbered `1. / 2. / ...` prefix matching execution order.
 * - Each entry trimmed of whitespace + truncated (ellipsis) to a
 *   single-line preview — the full text still runs when dequeued.
 * - When `selectedIndex` is provided (e.g. by a future keyboard-driven
 *   queue inspector), that row is highlighted with the accent color so
 *   the user can see which one they are about to edit.
 * - When `submissions.length > MAX_VISIBLE`, an `+N more` row is
 *   appended so the total is always visible at a glance.
 */

type Props = {
  submissions: readonly QueuedSubmission[]
  /**
   * Optional index of the "active" queued item — when set, that row
   * is rendered with the accent color. Out-of-range values are
   * silently ignored.
   */
  selectedIndex?: number
}

const MAX_VISIBLE = 3
const PREVIEW_CHARS = 60

export function QueuedSubmissions({ submissions, selectedIndex }: Props) {
  if (submissions.length === 0) return null

  const visible = submissions.slice(0, MAX_VISIBLE)
  const overflow = submissions.length - visible.length

  return (
    <box flexDirection="column" paddingLeft={3} paddingTop={1}>
      <text fg={c.dim}>Queued ({submissions.length}):</text>
      {visible.map((item, index) => {
        const selected = index === selectedIndex
        const preview = truncate(item.text.replace(/\s+/g, ' ').trim(), PREVIEW_CHARS)
        const prefix = `${index + 1}. `
        return (
          <box key={item.id} flexDirection="row" paddingLeft={1}>
            <text fg={selected ? c.accent : c.dim}>{prefix}</text>
            <text fg={selected ? c.text : c.dim}>{preview}</text>
          </box>
        )
      })}
      {overflow > 0 && (
        <box paddingLeft={1}>
          <text fg={c.muted}>+{overflow} more</text>
        </box>
      )}
    </box>
  )
}
