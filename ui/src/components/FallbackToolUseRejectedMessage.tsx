import React from 'react'
import { c } from '../theme.js'

/**
 * Generic "user rejected tool call" one-liner. Lite-native port of
 * `ui/examples/upstream-patterns/src/components/FallbackToolUseRejectedMessage.tsx`.
 *
 * Upstream wraps its `InterruptedByUser` atom in a single-row
 * `MessageResponse`. cc-rust doesn't ship either helper so we inline
 * their behaviour: a dim, selectable "User interrupted…" line with the
 * warning colour we already use for cancelled tool activity in
 * `ToolActivityMessage`.
 */
export function FallbackToolUseRejectedMessage() {
  return (
    <box flexDirection="row" paddingX={1} width="100%">
      <box minWidth={2} flexShrink={0}>
        <text fg={c.warning}>{'\u25A0'}</text>
      </box>
      <text fg={c.warning} selectable>
        User rejected the tool call.
      </text>
    </box>
  )
}
