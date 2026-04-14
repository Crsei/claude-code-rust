import React from 'react'
import type { ViewMode } from '../keybindings.js'
import { c } from '../theme.js'
import type { RenderItem } from '../store/message-model.js'
import { ThinkingBlock } from './ThinkingBlock.js'
import { ToolActivity } from './ToolActivity.js'
import { ToolGroup } from './ToolGroup.js'

interface Props {
  item: RenderItem
  viewMode: ViewMode
}

export function MessageBubble({ item, viewMode }: Props) {
  switch (item.type) {
    case 'user_text':
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

    case 'assistant_text':
      return (
        <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
          {item.thinking && <ThinkingBlock content={item.thinking} />}
          <box paddingLeft={1} flexDirection="column" width="100%" selectable>
            <markdown content={item.content} />
          </box>
        </box>
      )

    case 'system_text': {
      const levelColor = item.level === 'error'
        ? c.error
        : item.level === 'warning'
          ? c.warning
          : c.text
      const prefix = item.level === 'error'
        ? '[error]'
        : item.level === 'warning'
          ? '[warn]'
          : '[info]'

      return (
        <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
          <text selectable fg={levelColor}>
            {prefix} {item.content}
          </text>
        </box>
      )
    }

    case 'tool_activity':
      return <ToolActivity item={item} viewMode={viewMode} />

    case 'tool_group':
      return <ToolGroup item={item} />

    case 'tool_result_orphan': {
      const color = item.isError ? c.error : item.status === 'cancelled' ? c.warning : c.success
      const label = item.isError ? 'ORPHAN ERROR' : 'ORPHAN RESULT'
      return (
        <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
          <box gap={1}>
            <text fg={color}>
              <strong>[{label}]</strong>
            </text>
            <text fg={c.dim}>({item.toolUseId.slice(0, 8)})</text>
          </box>
          <box paddingLeft={2} width="100%">
            <text selectable fg={item.isError ? c.error : c.dim}>
              {item.outputSummary}
            </text>
          </box>
        </box>
      )
    }

    case 'streaming':
      return (
        <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
          {item.thinking && <ThinkingBlock content={item.thinking} />}
          <box paddingLeft={1} flexDirection="column" width="100%" selectable>
            {item.content ? <markdown content={item.content} streaming /> : <text fg={c.dim}>...</text>}
          </box>
        </box>
      )

    default:
      return null
  }
}
