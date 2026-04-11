import React from 'react'
import { Box, Text } from '../compat/ink-compat.js'

interface Props {
  output: string
  isError: boolean
  toolUseId: string
}

export function ToolResultBlock({ output, isError, toolUseId }: Props) {
  // Truncate very long outputs
  const maxLen = 2000
  const truncated = output.length > maxLen
  const displayText = truncated ? output.slice(0, maxLen) + '...' : output

  return (
    <Box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <Box gap={1}>
        <Text color={isError ? 'ansi:red' : 'ansi:green'} bold>
          {isError ? '✗' : '✓'} Result
        </Text>
        <Text dim>({toolUseId.slice(0, 8)})</Text>
      </Box>
      <Box paddingLeft={2} width="100%">
        <Text color={isError ? 'ansi:red' : undefined} dim={!isError} wrap="wrap">
          {displayText}
        </Text>
      </Box>
      {truncated && (
        <Box paddingLeft={2}>
          <Text dim italic>[{output.length} chars total, truncated]</Text>
        </Box>
      )}
    </Box>
  )
}
