import React from 'react'
import { c } from '../theme.js'
import type { UIMessage } from '../store/app-store.js'
import { ToolUseBlock } from './ToolUseBlock.js'
import { ToolResultBlock } from './ToolResultBlock.js'

interface Props {
  msg: UIMessage
}

export function MessageBubble({ msg }: Props) {
  switch (msg.role) {
    case 'user':
      return (
        <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
          <text><strong><span fg={c.user}>You</span></strong></text>
          <box paddingLeft={2} width="100%">
            <text selectable>{msg.content}</text>
          </box>
        </box>
      )

    case 'assistant':
      return (
        <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
          <text><strong><span fg={c.accent}>Assistant</span></strong></text>
          <box paddingLeft={2} flexDirection="column" width="100%">
            <markdown content={msg.content} />
          </box>
        </box>
      )

    case 'tool_use':
      return (
        <ToolUseBlock
          name={msg.toolName || 'unknown'}
          input={msg.toolInput}
          id={msg.id}
        />
      )

    case 'tool_result':
      return (
        <ToolResultBlock
          output={msg.content}
          isError={msg.isError || false}
          toolUseId={msg.id.replace('result-', '')}
        />
      )

    case 'system': {
      const levelColor = msg.level === 'error' ? c.error
        : msg.level === 'warning' ? c.warning
        : c.text
      const prefix = msg.level === 'error' ? '✗ ' : msg.level === 'warning' ? '⚠ ' : 'ℹ '

      return (
        <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
          <text fg={levelColor}>{prefix}{msg.content}</text>
        </box>
      )
    }

    default:
      return (
        <box paddingX={1} width="100%">
          <text fg={c.dim}>{msg.content}</text>
        </box>
      )
  }
}
