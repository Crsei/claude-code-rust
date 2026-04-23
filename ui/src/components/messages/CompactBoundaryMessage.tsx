import React from 'react'
import { c } from '../../theme.js'
import { shortcutLabel } from '../../keybindings.js'
import { useAppState } from '../../store/app-store.js'

/**
 * OpenTUI port of the upstream `CompactBoundaryMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/CompactBoundaryMessage.tsx`).
 *
 * Shown as a dim, centered separator whenever the backend signals that the
 * conversation was compacted — on the cc-rust IPC this arrives as a
 * `system_info` message with `level === 'compact_boundary'`. The optional
 * content from the backend supplies a one-line summary that replaces the
 * default label.
 */

type Props = {
  /** Optional summary line from the backend. Falls back to the default label. */
  content?: string
}

export function CompactBoundaryMessage({ content }: Props) {
  const { keybindingConfig } = useAppState()
  const shortcut = shortcutLabel('app:toggleTranscript', {
    context: 'Global',
    config: keybindingConfig,
  })
  const summary = content && content.trim() ? content.trim() : 'Conversation compacted'

  return (
    <box flexDirection="column" paddingX={1} marginTop={1} marginBottom={1} width="100%">
      <text fg={c.dim} bg={c.bg}>
        ✻ {summary} ({shortcut} for history)
      </text>
    </box>
  )
}
