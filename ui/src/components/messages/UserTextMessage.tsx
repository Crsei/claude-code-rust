import React from 'react'
import { c } from '../../theme.js'
import type { UserTextRenderItem } from '../../store/message-model.js'

/**
 * Lite-native user-text bubble. Re-hosts the visual shape of the sample
 * tree's `UserPromptMessage` / `UserTextMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/UserPromptMessage.tsx`)
 * but consumes the local `UserTextRenderItem` shape so it plugs straight
 * into the existing `buildRenderItems` pipeline.
 */
type Props = {
  item: UserTextRenderItem
}

export function UserTextMessage({ item }: Props) {
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box
        flexDirection="column"
        width="100%"
        border={['left']}
        borderColor={c.userBubbleBorder}
        backgroundColor={c.userBubbleBg}
        paddingLeft={1}
        paddingRight={1}
        selectable
      >
        <markdown content={item.content} />
      </box>
    </box>
  )
}
