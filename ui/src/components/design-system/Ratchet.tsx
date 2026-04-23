import React from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `Ratchet` — a stepped progress
 * indicator rendered as a row of filled / empty blocks. Unlike
 * `<ProgressBar>` the filled count is already discrete (no ratio
 * math). Used by dialog chromes to show "step 2 of 5" style
 * affordances.
 */

type Props = {
  step: number
  total: number
  label?: string
  /** Colour of the completed pip. */
  color?: string
  /** Colour of the upcoming pip. */
  dimColor?: string
  /** Show `1 of 5` text after the pips. */
  showCount?: boolean
}

const DONE = '\u25C6'
const TODO = '\u25C7'

export function Ratchet({
  step,
  total,
  label,
  color,
  dimColor,
  showCount = true,
}: Props) {
  const safeTotal = Math.max(1, total)
  const safeStep = Math.max(0, Math.min(safeTotal, step))
  const done = color ?? c.success
  const todo = dimColor ?? c.dim

  return (
    <box flexDirection="row" gap={1}>
      {label && <text>{label}</text>}
      <box flexDirection="row">
        {Array.from({ length: safeTotal }).map((_, i) => {
          const isDone = i < safeStep
          return (
            <text key={i} fg={isDone ? done : todo}>
              {isDone ? DONE : TODO}
              {i < safeTotal - 1 ? ' ' : ''}
            </text>
          )
        })}
      </box>
      {showCount && (
        <text fg={c.dim}>
          {safeStep} of {safeTotal}
        </text>
      )}
    </box>
  )
}
