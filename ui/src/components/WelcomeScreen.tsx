import React from 'react'
import { Box, Text } from 'ink-terminal'
import { useAppState } from '../store/app-store.js'

const LOGO = `
   ___  ___        ___  _   _  ___ _____
  / __|/ __|___   | _ \\| | | |/ __|_   _|
 | (__| (__|___|  |   /| |_| |\\__ \\ | |
  \\___|\\___|      |_|_\\ \\___/ |___/ |_|
`

export function WelcomeScreen() {
  const { model, cwd, sessionId } = useAppState()

  return (
    <Box flexDirection="column" alignItems="center">
      <Text color="ansi:magenta" bold>{LOGO}</Text>
      <Box flexDirection="column" gap={0} paddingX={2}>
        <Text>
          <Text dim>Model: </Text>
          <Text bold>{model || 'connecting...'}</Text>
        </Text>
        <Text>
          <Text dim>  cwd: </Text>
          <Text>{cwd || '...'}</Text>
        </Text>
        {sessionId && (
          <Text>
            <Text dim>  Session: </Text>
            <Text dim>{sessionId.slice(0, 8)}</Text>
          </Text>
        )}
      </Box>
      <Box marginTop={1}>
        <Text dim italic>Type a message to get started. Ctrl+D to quit.</Text>
      </Box>
    </Box>
  )
}
