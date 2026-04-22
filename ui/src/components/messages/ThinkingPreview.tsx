import React from 'react'
import { c } from '../../theme.js'

/**
 * Small header + preview shown above assistant or streaming text when the
 * underlying segment carries thinking content. Lite-native sibling of the
 * sample tree's `AssistantThinkingMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/AssistantThinkingMessage.tsx`)
 * but pared down to the one bubble-preview slot the Lite shell actually
 * surfaces today.
 *
 * Kept as a leaf under `ui/src/components/messages/` so it can be reused
 * from multiple parent message types (assistant + streaming) without
 * round-tripping through `MessageBubble`.
 */

const PREVIEW_LIMIT = 100

type Props = {
  content: string
}

export function ThinkingPreview({ content }: Props) {
  const preview = content.length > PREVIEW_LIMIT
    ? `${content.slice(0, PREVIEW_LIMIT)}\u2026`
    : content

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1}>
      <text>
        <em>
          <span fg={c.dim}>[thinking] {content.length} chars</span>
        </em>
      </text>
      <box paddingLeft={2}>
        <text>
          <em>
            <span fg={c.dim}>{preview}</span>
          </em>
        </text>
      </box>
    </box>
  )
}
