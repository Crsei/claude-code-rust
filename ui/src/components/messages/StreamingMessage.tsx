import React from 'react'
import { c } from '../../theme.js'
import type { StreamingRenderItem } from '../../store/message-model.js'
import { ThinkingPreview } from './ThinkingPreview.js'

/**
 * Lite-native rendering of the in-flight streaming assistant segment.
 * Distinct from `AssistantTextMessage` because (a) the markdown is
 * rendered with `streaming` so partial tokens don't trigger layout
 * jitter, and (b) we show a dim `...` placeholder while the first tokens
 * are still in flight.
 */
type Props = {
  item: StreamingRenderItem
}

export function StreamingMessage({ item }: Props) {
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      {item.thinking && <ThinkingPreview content={item.thinking} />}
      <box paddingLeft={1} flexDirection="column" width="100%" selectable backgroundColor={c.bg}>
        {item.content
          ? <markdown content={item.content} streaming bg={c.bg} />
          : <text fg={c.dim} bg={c.bg}>...</text>}
      </box>
    </box>
  )
}
