import React from 'react'
import { Box, Text, Spacer } from 'ink-terminal'
import type { Usage } from '../store/app-store.js'
import { formatCost, formatTokens } from '../utils.js'

export function StatusBar({ usage, model, vimMode }: { usage: Usage; model: string; vimMode?: string }) {
  return (
    <Box paddingX={1} borderStyle="single" borderTop borderBottom={false} borderLeft={false} borderRight={false}>
      {vimMode && (
        <>
          <Text color="ansi:yellow" bold>[{vimMode}]</Text>
          <Text dim> | </Text>
        </>
      )}
      <Text dim>Model: </Text>
      <Text>{model}</Text>
      <Spacer />
      <Text dim>Tokens: </Text>
      <Text>{formatTokens(usage.inputTokens + usage.outputTokens)}</Text>
      <Text dim> | Cost: </Text>
      <Text color="ansi:green">{formatCost(usage.costUsd)}</Text>
    </Box>
  )
}
