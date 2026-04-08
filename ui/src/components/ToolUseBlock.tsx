import React, { useState } from 'react'
import { Box, Text } from 'ink-terminal'

interface Props {
  name: string
  input: any
  id: string
}

export function ToolUseBlock({ name, input, id }: Props) {
  const [expanded, setExpanded] = useState(false)

  // Format input for display
  const inputStr = typeof input === 'string' ? input : JSON.stringify(input, null, 2)
  const isLong = inputStr.length > 200

  return (
    <Box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <Box gap={1}>
        <Text color="ansi:yellow" bold>{'⚡'} {name}</Text>
        <Text dim>({id.slice(0, 8)})</Text>
      </Box>
      <Box paddingLeft={2} flexDirection="column" width="100%">
        {isLong && !expanded ? (
          <>
            <Text dim wrap="wrap">{inputStr.slice(0, 200)}...</Text>
            <Text dim italic>[{inputStr.length} chars, truncated]</Text>
          </>
        ) : (
          <Text dim wrap="wrap">{inputStr}</Text>
        )}
      </Box>
    </Box>
  )
}
