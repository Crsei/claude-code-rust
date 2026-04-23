import React, { useEffect, useState } from 'react'
import { c } from '../theme.js'
import { formatCost, formatTokens } from '../utils.js'

/**
 * Built-in statusline renderer focused on model / context usage /
 * rate-limit meters + session cost.
 *
 * OpenTUI-native port of the upstream `BuiltinStatusLine`
 * (`ui/examples/upstream-patterns/src/components/BuiltinStatusLine.tsx`).
 * The existing repository-side `components/StatusLine/BuiltinStatusLine.tsx`
 * renders a different surface (cwd, model, agent/team/MCP/LSP counts)
 * for the main status bar; this component keeps the upstream's
 * rate-limit-aware layout intact so any surface that needs that view
 * (e.g. a custom statusline configuration) has an in-tree port.
 */

export type RateLimitBucket = {
  /** 0.0 \u2013 1.0 */
  utilization: number
  /** Epoch seconds (0 / negative to hide). */
  resets_at: number
}

export type BuiltinStatusLineProps = {
  modelName: string
  contextUsedPct: number
  usedTokens: number
  contextWindowSize: number
  totalCostUsd: number
  rateLimits: {
    five_hour?: RateLimitBucket
    seven_day?: RateLimitBucket
  }
  /** Available terminal columns. Defaults to 100. */
  columns?: number
}

export function formatCountdown(epochSeconds: number): string {
  const diff = epochSeconds - Date.now() / 1000
  if (diff <= 0) return 'now'
  const days = Math.floor(diff / 86400)
  const hours = Math.floor((diff % 86400) / 3600)
  const minutes = Math.floor((diff % 3600) / 60)
  if (days >= 1) return `${days}d${hours}h`
  if (hours >= 1) return `${hours}h${minutes}m`
  return `${minutes}m`
}

function progressBar(ratio: number, width: number): string {
  const safe = Math.max(0, Math.min(1, ratio))
  const filledCells = Math.round(safe * width)
  return '\u2588'.repeat(filledCells) + '\u2591'.repeat(Math.max(0, width - filledCells))
}

function Separator() {
  return <span fg={c.dim}>{' \u2502 '}</span>
}

function BuiltinStatusLineInner({
  modelName,
  contextUsedPct,
  usedTokens,
  contextWindowSize,
  totalCostUsd,
  rateLimits,
  columns = 100,
}: BuiltinStatusLineProps) {
  const [, setTick] = useState(0)
  useEffect(() => {
    const hasResetTime =
      (rateLimits.five_hour?.resets_at ?? 0) ||
      (rateLimits.seven_day?.resets_at ?? 0)
    if (!hasResetTime) return
    const id = setInterval(() => setTick(t => t + 1), 60_000)
    return () => clearInterval(id)
  }, [rateLimits.five_hour?.resets_at, rateLimits.seven_day?.resets_at])

  const modelParts = modelName.split(' ')
  const shortModel =
    modelParts.length >= 2 ? `${modelParts[0]} ${modelParts[1]}` : modelName

  const wide = columns >= 100
  const narrow = columns < 60

  const hasFiveHour = rateLimits.five_hour != null
  const hasSevenDay = rateLimits.seven_day != null

  const fiveHourPct = hasFiveHour
    ? Math.round((rateLimits.five_hour?.utilization ?? 0) * 100)
    : 0
  const sevenDayPct = hasSevenDay
    ? Math.round((rateLimits.seven_day?.utilization ?? 0) * 100)
    : 0

  const tokenDisplay = `${formatTokens(usedTokens)}/${formatTokens(contextWindowSize)}`

  return (
    <text>
      {shortModel}
      <Separator />
      <span fg={c.dim}>Context </span>
      {contextUsedPct}%
      {!narrow && <span fg={c.dim}>{` (${tokenDisplay})`}</span>}
      {hasFiveHour && (
        <>
          <Separator />
          <span fg={c.dim}>Session </span>
          {wide && (
            <>
              <span fg={c.warning}>
                {progressBar(rateLimits.five_hour?.utilization ?? 0, 10)}
              </span>
              {' '}
            </>
          )}
          {fiveHourPct}%
          {!narrow && (rateLimits.five_hour?.resets_at ?? 0) > 0 && (
            <span fg={c.dim}>{` ${formatCountdown(rateLimits.five_hour!.resets_at)}`}</span>
          )}
        </>
      )}
      {hasSevenDay && (
        <>
          <Separator />
          <span fg={c.dim}>Weekly </span>
          {wide && (
            <>
              <span fg={c.warning}>
                {progressBar(rateLimits.seven_day?.utilization ?? 0, 10)}
              </span>
              {' '}
            </>
          )}
          {sevenDayPct}%
          {!narrow && (rateLimits.seven_day?.resets_at ?? 0) > 0 && (
            <span fg={c.dim}>{` ${formatCountdown(rateLimits.seven_day!.resets_at)}`}</span>
          )}
        </>
      )}
      {totalCostUsd > 0 && (
        <>
          <Separator />
          {formatCost(totalCostUsd)}
        </>
      )}
    </text>
  )
}

export const BuiltinStatusLine = React.memo(BuiltinStatusLineInner)
