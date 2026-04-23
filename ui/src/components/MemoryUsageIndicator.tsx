import React, { useEffect, useState } from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `MemoryUsageIndicator`
 * (`ui/examples/upstream-patterns/src/components/MemoryUsageIndicator.tsx`).
 *
 * Polls Node/Bun's heap metrics on a 10-second cadence and surfaces a
 * warning line when heap usage crosses the "high"/"critical" thresholds
 * upstream uses. Upstream gates the entire widget behind
 * `USER_TYPE === 'ant'` (internal-only). cc-rust has no equivalent flag,
 * so we expose the check via the `CC_RUST_MEMORY_INDICATOR` env var so
 * operators can opt in without rebuilding.
 */

const HIGH_THRESHOLD = 0.75
const CRITICAL_THRESHOLD = 0.9
const POLL_INTERVAL_MS = 10_000

type Status = 'normal' | 'high' | 'critical'

interface MemoryUsage {
  heapUsed: number
  status: Status
}

function formatFileSize(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit++
  }
  return `${value.toFixed(1)} ${units[unit]}`
}

function readMemoryUsage(): MemoryUsage | null {
  if (typeof process === 'undefined' || typeof process.memoryUsage !== 'function') {
    return null
  }
  try {
    const mu = process.memoryUsage()
    const total = mu.heapTotal || 1
    const ratio = mu.heapUsed / total
    let status: Status = 'normal'
    if (ratio >= CRITICAL_THRESHOLD) status = 'critical'
    else if (ratio >= HIGH_THRESHOLD) status = 'high'
    return { heapUsed: mu.heapUsed, status }
  } catch {
    return null
  }
}

export function MemoryUsageIndicator() {
  const enabled =
    typeof process !== 'undefined' &&
    process.env?.CC_RUST_MEMORY_INDICATOR === '1'
  const [usage, setUsage] = useState<MemoryUsage | null>(null)

  useEffect(() => {
    if (!enabled) return
    const tick = () => setUsage(readMemoryUsage())
    tick()
    const id = setInterval(tick, POLL_INTERVAL_MS)
    return () => clearInterval(id)
  }, [enabled])

  if (!enabled || !usage || usage.status === 'normal') {
    return null
  }

  const color = usage.status === 'critical' ? c.error : c.warning
  return (
    <box>
      <text fg={color}>
        High memory usage ({formatFileSize(usage.heapUsed)})
      </text>
    </box>
  )
}
