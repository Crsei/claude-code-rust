import React from 'react'
import { c } from '../../theme.js'
import type { StreamingRenderItem } from '../../store/message-model.js'
import { ThinkingPreview } from './ThinkingPreview.js'

/**
 * In-flight streaming segment — OpenTUI sibling of `AssistantTextMessage`
 * that (a) passes `streaming` to the markdown renderer to avoid layout
 * jitter on partial tokens, and (b) shows a dim `...` placeholder while
 * the first tokens are still in flight.
 *
 * Matches the upstream `AssistantTextMessage` visual shape (leading `●`
 * anchor glyph + indented body) so the bubble geometry doesn't shift
 * when the streaming segment is replaced by a finalised one.
 */
type Props = {
  item: StreamingRenderItem
}

export function StreamingMessage({ item }: Props) {
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      {item.thinking && <ThinkingPreview content={item.thinking} />}
      <box flexDirection="row" width="100%">
        <box minWidth={2} flexShrink={0}>
          <text fg={c.dim} bg={c.bg}>●</text>
        </box>
        <box flexDirection="column" width="100%" selectable backgroundColor={c.bg}>
          {item.content
            ? <markdown content={item.content} streaming bg={c.bg} />
            : <text fg={c.dim} bg={c.bg}>...</text>}
        </box>
      </box>
    </box>
  )
}
