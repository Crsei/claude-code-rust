import React, { useEffect, useState } from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `LoadingState` component.
 *
 * A rotating braille spinner followed by a label. The spinner animates
 * at 80 ms / frame (same cadence as the existing `Spinner` component)
 * and honours the `dimColor` prop by rendering the whole line in the
 * dim theme colour.
 */

const FRAMES = [
  '\u280B',
  '\u2819',
  '\u2839',
  '\u2838',
  '\u283C',
  '\u2834',
  '\u2826',
  '\u2827',
  '\u2807',
  '\u280F',
]

type Props = {
  message?: string
  dimColor?: boolean
  /** Override foreground colour for the spinner glyph. */
  color?: string
}

export function LoadingState({ message = 'Loading…', dimColor = false, color }: Props) {
  const [frame, setFrame] = useState(0)

  useEffect(() => {
    const id = setInterval(() => {
      setFrame(f => (f + 1) % FRAMES.length)
    }, 80)
    return () => clearInterval(id)
  }, [])

  const textColor = dimColor ? c.dim : c.text
  const spinnerColor = color ?? (dimColor ? c.dim : c.info)

  return (
    <box flexDirection="row" gap={1}>
      <text fg={spinnerColor}>{FRAMES[frame]}</text>
      <text fg={textColor}>{message}</text>
    </box>
  )
}
