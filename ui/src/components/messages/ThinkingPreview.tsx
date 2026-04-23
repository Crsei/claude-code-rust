import React from 'react'
import { c } from '../../theme.js'

/**
 * Thinking-content header shown above assistant / streaming text when the
 * underlying segment carries extended-thinking output. OpenTUI sibling of
 * upstream `AssistantThinkingMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/AssistantThinkingMessage.tsx`).
 *
 * Mirrors upstream's `∴ Thinking…` label style (dim italic) and indents the
 * preview under it. Long thinking is truncated with an ellipsis — we keep
 * the full text in-store so the transcript view can still expand it.
 */

const PREVIEW_LIMIT = 100

type Props = {
  content: string
}

export function ThinkingPreview({ content }: Props) {
  const trimmed = content.trim()
  if (!trimmed) {
    return null
  }
  const preview = trimmed.length > PREVIEW_LIMIT
    ? `${trimmed.slice(0, PREVIEW_LIMIT)}\u2026`
    : trimmed

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1}>
      <text fg={c.dim}>
        <em>{'\u2234 Thinking\u2026'}</em>
      </text>
      <box paddingLeft={2}>
        <text fg={c.dim}>
          <em>{preview}</em>
        </text>
      </box>
    </box>
  )
}
