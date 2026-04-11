import React from 'react'
import { Box, Text } from '../compat/ink-compat.js'
import { useAppState } from '../store/app-store.js'

export function Suggestions() {
  const { suggestions } = useAppState()

  if (suggestions.length === 0) return null

  return (
    <Box flexDirection="column" paddingX={1} paddingY={0}>
      <Text dim italic>Suggestions:</Text>
      {suggestions.map((s, i) => (
        <Box key={i} paddingLeft={2}>
          <Text dim color="ansi:cyan">{i + 1}. </Text>
          <Text dim>{s}</Text>
        </Box>
      ))}
    </Box>
  )
}
