import React from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `ProgressBar`. Draws a fixed-width
 * bar using solid / light block characters, with optional label and
 * percentage. `value` and `max` are clamped so callers don't need to.
 */

type Props = {
  value: number
  max?: number
  /** Character cells wide (inside any surrounding padding). */
  width?: number
  label?: string
  /** When true, renders `42%` after the bar. */
  showPercent?: boolean
  /** Colour of the filled portion. */
  color?: string
  /** Colour of the unfilled portion. */
  dimColor?: string
}

const FILLED = '\u2588'
const EMPTY = '\u2591'

export function ProgressBar({
  value,
  max = 100,
  width = 20,
  label,
  showPercent = false,
  color,
  dimColor,
}: Props) {
  const safeMax = Math.max(1, max)
  const ratio = Math.max(0, Math.min(1, value / safeMax))
  const filledCount = Math.round(ratio * width)
  const emptyCount = Math.max(0, width - filledCount)
  const filledColor = color ?? c.success
  const emptyColor = dimColor ?? c.dim

  return (
    <box flexDirection="row" gap={1}>
      {label && <text>{label}</text>}
      <text fg={filledColor}>{FILLED.repeat(filledCount)}</text>
      <text fg={emptyColor}>{EMPTY.repeat(emptyCount)}</text>
      {showPercent && (
        <text fg={c.dim}>{Math.round(ratio * 100)}%</text>
      )}
    </box>
  )
}
