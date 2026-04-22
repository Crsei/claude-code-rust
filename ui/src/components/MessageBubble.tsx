import React from 'react'
import type { ViewMode } from '../keybindings.js'
import type { RenderItem } from '../store/message-model.js'
import {
  AssistantTextMessage,
  StreamingMessage,
  SystemMessage,
  ToolActivityMessage,
  ToolGroupMessage,
  ToolResultOrphanMessage,
  UserTextMessage,
} from './messages/index.js'

/**
 * Thin dispatcher that picks the right leaf component from
 * `./messages/` for each `RenderItem` discriminant. Each discriminant
 * has exactly one leaf, so adding a new item type means adding one
 * branch here and one file under `./messages/`.
 */

interface Props {
  item: RenderItem
  viewMode: ViewMode
}

export function MessageBubble({ item, viewMode }: Props) {
  switch (item.type) {
    case 'user_text':
      return <UserTextMessage item={item} />
    case 'assistant_text':
      return <AssistantTextMessage item={item} />
    case 'system_text':
      return <SystemMessage item={item} />
    case 'tool_activity':
      return <ToolActivityMessage item={item} viewMode={viewMode} />
    case 'tool_group':
      return <ToolGroupMessage item={item} />
    case 'tool_result_orphan':
      return <ToolResultOrphanMessage item={item} />
    case 'streaming':
      return <StreamingMessage item={item} />
    default:
      return null
  }
}
