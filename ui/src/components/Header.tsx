import React from 'react'
import { Box, Text } from '../compat/ink-compat.js'

export function Header({ model, sessionId }: { model: string; sessionId: string }) {
  return (
    <Box paddingX={1} borderStyle="single" borderBottom borderTop={false} borderLeft={false} borderRight={false}>
      <Text bold color="ansi:magenta">cc-rust</Text>
      <Text dim> | </Text>
      <Text>{model}</Text>
      {sessionId && (
        <>
          <Text dim> | </Text>
          <Text dim>{sessionId.slice(0, 8)}</Text>
        </>
      )}
    </Box>
  )
}
