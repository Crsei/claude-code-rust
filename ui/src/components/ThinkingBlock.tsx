import React, { useState } from 'react'
import { Box, Text } from '../compat/ink-compat.js'

interface Props {
  content: string
}

export function ThinkingBlock({ content }: Props) {
  const [expanded, setExpanded] = useState(false)

  // Show first 100 chars as preview when collapsed
  const preview = content.length > 100 ? content.slice(0, 100) + '...' : content

  return (
    <Box flexDirection="column" paddingX={1} marginBottom={1}>
      <Text dim italic>
        {'💭'} Thinking {expanded ? '▼' : '▶'} {!expanded && `(${content.length} chars)`}
      </Text>
      {expanded ? (
        <Box paddingLeft={2}>
          <Text dim italic>{content}</Text>
        </Box>
      ) : (
        <Box paddingLeft={2}>
          <Text dim italic>{preview}</Text>
        </Box>
      )}
    </Box>
  )
}
