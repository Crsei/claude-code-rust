import React from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `KeyboardShortcutHint`.
 *
 * Upstream renders `<kbd>…</kbd>` with a bold shortcut label followed
 * by a dim action description, e.g. `Enter confirm`. The layout is a
 * single inline text run so it can be nested under `<Byline>` to build
 * status-bar-style chromes.
 */

type Props = {
  /** Key or chord ("Enter", "Ctrl+B", "↑/↓"). */
  shortcut: string
  /** Short verb label ("confirm", "cancel"). */
  action?: string
  /** Override foreground colour. */
  color?: string
  /** When true, render the shortcut label in bold. */
  bold?: boolean
}

export function KeyboardShortcutHint({
  shortcut,
  action,
  color,
  bold = true,
}: Props) {
  const fg = color ?? c.text
  const shortcutEl = bold ? (
    <strong><span fg={fg}>{shortcut}</span></strong>
  ) : (
    <span fg={fg}>{shortcut}</span>
  )
  return (
    <text>
      {shortcutEl}
      {action && (
        <span fg={c.dim}>{' '}{action}</span>
      )}
    </text>
  )
}
