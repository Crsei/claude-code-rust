import React from 'react'
import { Box, Text } from 'ink-terminal'

interface Props {
  diff: string
}

export function DiffView({ diff }: Props) {
  const lines = diff.split('\n')

  return (
    <Box flexDirection="column" borderStyle="round" borderColor="ansi:white" borderDimColor paddingX={1}>
      {lines.map((line, i) => {
        let color: string | undefined
        let dim = false

        if (line.startsWith('+++') || line.startsWith('---')) {
          dim = true
        } else if (line.startsWith('+')) {
          color = 'ansi:green'
        } else if (line.startsWith('-')) {
          color = 'ansi:red'
        } else if (line.startsWith('@@')) {
          color = 'ansi:cyan'
          dim = true
        }

        return (
          <Text key={i} color={color as any} dim={dim}>
            {line}
          </Text>
        )
      })}
    </Box>
  )
}
