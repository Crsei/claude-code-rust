import React from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/AgentNavigationFooter.tsx`.
 *
 * Single dim line at the bottom of the agents menu explaining
 * navigation. Upstream couples this with an Ink `useExitOnCtrlCD` hook
 * that flips the label to "Press Ctrl+C again to exit"; the Lite
 * wiring for that shortcut lives higher up in `App.tsx`, so this
 * component is purely presentational.
 */

type Props = {
  instructions?: string
  /** When set, overrides the label with a pending-exit hint. */
  exitPendingLabel?: string
}

export function AgentNavigationFooter({
  instructions = 'Press \u2191\u2193 to navigate · Enter to select · Esc to go back',
  exitPendingLabel,
}: Props) {
  return (
    <box marginLeft={2}>
      <text fg={c.dim}>{exitPendingLabel ?? instructions}</text>
    </box>
  )
}
