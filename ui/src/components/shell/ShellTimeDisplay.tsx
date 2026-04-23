import React from 'react'
import { c } from '../../theme.js'
import { formatDuration } from './format.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/shell/ShellTimeDisplay.tsx`.
 *
 * Renders a compact "(1s)", "(1s · timeout 30s)", or "(timeout 30s)"
 * annotation for a shell run — kept next to the progress/output block so
 * the user knows how long the command has been running and when it will
 * be killed.
 */

type Props = {
  elapsedTimeSeconds?: number
  timeoutMs?: number
}

export function ShellTimeDisplay({ elapsedTimeSeconds, timeoutMs }: Props) {
  if (elapsedTimeSeconds === undefined && !timeoutMs) {
    return null
  }

  const timeout = timeoutMs
    ? formatDuration(timeoutMs, { hideTrailingZeros: true })
    : undefined

  if (elapsedTimeSeconds === undefined) {
    return <text fg={c.dim}>{`(timeout ${timeout})`}</text>
  }

  const elapsed = formatDuration(elapsedTimeSeconds * 1000)
  if (timeout) {
    return <text fg={c.dim}>{`(${elapsed} · timeout ${timeout})`}</text>
  }
  return <text fg={c.dim}>{`(${elapsed})`}</text>
}
