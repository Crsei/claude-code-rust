import React from 'react'
import { Box, Text } from 'ink-terminal'
import { Markdown } from 'ink-terminal/markdown'
import type { UIMessage } from '../store/app-store.js'
import { ToolUseBlock } from './ToolUseBlock.js'
import { ToolResultBlock } from './ToolResultBlock.js'
import { ThinkingBlock } from './ThinkingBlock.js'

interface Props {
  msg: UIMessage
}

export function MessageBubble({ msg }: Props) {
  switch (msg.role) {
    case 'user':
      return (
        <Box flexDirection="column" paddingX={1} marginBottom={1}>
          <Text color="ansi:cyanBright" bold>You</Text>
          <Box paddingLeft={2}>
            <Text>{msg.content}</Text>
          </Box>
        </Box>
      )

    case 'assistant':
      return (
        <Box flexDirection="column" paddingX={1} marginBottom={1}>
          <Text color="ansi:magenta" bold>Assistant</Text>
          <Box paddingLeft={2} flexDirection="column">
            <Markdown>{msg.content}</Markdown>
          </Box>
        </Box>
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
      const levelColor = msg.level === 'error' ? 'ansi:red'
        : msg.level === 'warning' ? 'ansi:yellow'
        : 'ansi:white'

      return (
        <Box flexDirection="column" paddingX={1} marginBottom={1}>
          <Text color={levelColor as any} dim={msg.level !== 'error'}>
            {msg.level === 'error' ? '✗ ' : msg.level === 'warning' ? '⚠ ' : 'ℹ '}
            {msg.content}
          </Text>
        </Box>
      )
    }

    default:
      return (
        <Box paddingX={1}>
          <Text dim>{msg.content}</Text>
        </Box>
      )
  }
}
