import React from 'react'
import type { AssistantTextRenderItem } from '../../store/message-model.js'
import { c } from '../../theme.js'
import { ThinkingPreview } from './ThinkingPreview.js'

/**
 * Lite-native assistant-text bubble. Mirrors the decomposition used by
 * the sample tree's `AssistantTextMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/AssistantTextMessage.tsx`)
 * — thinking preview on top, markdown body underneath — but without the
 * Ink-only error-path branches that are handled elsewhere in the Lite
 * shell.
 */
type Props = {
  item: AssistantTextRenderItem
}

export function AssistantTextMessage({ item }: Props) {
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      {item.thinking && <ThinkingPreview content={item.thinking} />}
      <box paddingLeft={1} flexDirection="column" width="100%" selectable backgroundColor={c.bg}>
        <markdown content={item.content} bg={c.bg} />
      </box>
    </box>
  )
}
