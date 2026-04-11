import React from 'react'
import { Box, Text, useAnimationFrame } from '../compat/ink-compat.js'

const FRAMES = ['\u280B', '\u2819', '\u2839', '\u2838', '\u283C', '\u2834', '\u2826', '\u2827', '\u2807', '\u280F']

export function Spinner({ label = 'Thinking...' }: { label?: string }) {
  const [ref, time] = useAnimationFrame(80)
  const frame = Math.floor(time / 80) % FRAMES.length

  return (
    <Box ref={ref} paddingX={1}>
      <Text color="ansi:cyan">{FRAMES[frame]} {label}</Text>
    </Box>
  )
}
