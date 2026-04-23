import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/Settings/Usage.tsx`.
 *
 * Upstream calls `fetchUtilization()` against the Anthropic API and
 * paints three rate-limit bars (5-hour, 7-day, 7-day Sonnet) plus an
 * "Extra usage" section for Pro/Max. cc-rust does not forward
 * utilization through IPC yet, so this component exposes the same
 * surface as a pluggable `loadUtilization` prop:
 *
 * - Default `loadUtilization` returns `null`, which renders the
 *   "Usage is only available for subscription plans." placeholder —
 *   matching upstream's behaviour when the API returns no limits.
 * - Hosts can swap in a real loader (e.g. via a new IPC command) to
 *   get the three bars without touching the renderer.
 *
 * The progress bar is rendered with a simple `[====    ]` ASCII
 * widget — OpenTUI doesn't ship Ink's `<ProgressBar>`.
 */

export interface RateLimit {
  /** 0-100 — progress bar fill ratio. `null` means the API didn't
   *  return this limit (e.g. free tier has no Sonnet bar). */
  utilization: number | null
  /** ISO-8601 reset timestamp. */
  resets_at?: string | null
}

export interface ExtraUsage {
  is_enabled: boolean
  monthly_limit: number | null
  used_credits?: number
  utilization?: number
}

export interface Utilization {
  five_hour: RateLimit | null
  seven_day: RateLimit | null
  seven_day_sonnet: RateLimit | null
  extra_usage?: ExtraUsage | null
}

type Props = {
  /** Override the loader. Defaults to a stub that returns `null`. */
  loadUtilization?: () => Promise<Utilization | null>
  /** Display-name override — matches upstream's
   *  `getSubscriptionType()` gating for the Sonnet / Extra rows. */
  subscriptionType?: 'pro' | 'max' | 'team' | 'free' | null
}

const DEFAULT_LOAD = async (): Promise<Utilization | null> => null

export function Usage({
  loadUtilization = DEFAULT_LOAD,
  subscriptionType = null,
}: Props = {}) {
  const [utilization, setUtilization] = useState<Utilization | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const load = async () => {
    setIsLoading(true)
    setError(null)
    try {
      const data = await loadUtilization()
      setUtilization(data)
    } catch (e) {
      setError(
        e instanceof Error ? e.message : 'Failed to load usage data',
      )
    } finally {
      setIsLoading(false)
    }
  }

  useEffect(() => {
    void load()
    // The loader is intentionally not listed in the dep array; retry
    // is driven by the 'r' keyboard handler below, same as upstream.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (!error || isLoading) return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()
    if (input === 'r') void load()
  })

  if (error) {
    return (
      <box flexDirection="column" gap={1} paddingY={1}>
        <text fg={c.error} selectable>Error: {error}</text>
        <text fg={c.dim}>
          <em>Press `r` to retry · Esc to cancel</em>
        </text>
      </box>
    )
  }

  if (isLoading) {
    return (
      <box flexDirection="column" gap={1} paddingY={1}>
        <text fg={c.dim}>Loading usage data…</text>
      </box>
    )
  }

  if (!utilization) {
    return (
      <box flexDirection="column" gap={1} paddingY={1}>
        <text fg={c.dim}>
          /usage is only available for subscription plans.
        </text>
      </box>
    )
  }

  const showSonnetBar =
    subscriptionType === 'max' ||
    subscriptionType === 'team' ||
    subscriptionType === null
  const limits: Array<{ title: string; limit: RateLimit | null }> = [
    { title: 'Current session', limit: utilization.five_hour },
    { title: 'Current week (all models)', limit: utilization.seven_day },
  ]
  if (showSonnetBar) {
    limits.push({
      title: 'Current week (Sonnet only)',
      limit: utilization.seven_day_sonnet,
    })
  }

  const hasAnyLimit = limits.some(({ limit }) => limit && limit.utilization !== null)

  return (
    <box flexDirection="column" gap={1} paddingY={1} width="100%">
      {!hasAnyLimit && (
        <text fg={c.dim}>/usage is only available for subscription plans.</text>
      )}
      {limits.map(({ title, limit }) =>
        limit && limit.utilization !== null ? (
          <LimitBar key={title} title={title} limit={limit} />
        ) : null,
      )}
      {utilization.extra_usage &&
        (subscriptionType === 'pro' || subscriptionType === 'max') && (
          <ExtraUsageSection extraUsage={utilization.extra_usage} />
        )}
      <text fg={c.dim}>
        <em>Esc to cancel</em>
      </text>
    </box>
  )
}

function LimitBar({
  title,
  limit,
  extraSubtext,
}: {
  title: string
  limit: RateLimit
  extraSubtext?: string
}) {
  const util = limit.utilization ?? 0
  const barWidth = 40
  const filled = Math.max(0, Math.min(barWidth, Math.round((util / 100) * barWidth)))
  const bar = '\u2588'.repeat(filled) + '\u2591'.repeat(barWidth - filled)
  const usedText = `${Math.floor(util)}% used`
  const subtext = buildSubtext(limit.resets_at ?? null, extraSubtext)

  return (
    <box flexDirection="column">
      <text>
        <strong>{title}</strong>
      </text>
      <box flexDirection="row" gap={1}>
        <text fg={util >= 90 ? c.warning : c.accent}>{bar}</text>
        <text>{usedText}</text>
      </box>
      {subtext && <text fg={c.dim}>{subtext}</text>}
    </box>
  )
}

function ExtraUsageSection({ extraUsage }: { extraUsage: ExtraUsage }) {
  if (!extraUsage.is_enabled) {
    return (
      <box flexDirection="column">
        <text>
          <strong>Extra usage</strong>
        </text>
        <text fg={c.dim}>Extra usage not enabled · /extra-usage to enable</text>
      </box>
    )
  }
  if (extraUsage.monthly_limit === null) {
    return (
      <box flexDirection="column">
        <text>
          <strong>Extra usage</strong>
        </text>
        <text fg={c.dim}>Unlimited</text>
      </box>
    )
  }
  if (
    typeof extraUsage.used_credits !== 'number' ||
    typeof extraUsage.utilization !== 'number'
  ) {
    return null
  }
  const used = (extraUsage.used_credits / 100).toFixed(2)
  const cap = (extraUsage.monthly_limit / 100).toFixed(2)
  const oneMonthReset = new Date()
  oneMonthReset.setMonth(oneMonthReset.getMonth() + 1, 1)
  return (
    <LimitBar
      title="Extra usage"
      limit={{
        utilization: extraUsage.utilization,
        resets_at: oneMonthReset.toISOString(),
      }}
      extraSubtext={`$${used} / $${cap} spent`}
    />
  )
}

function buildSubtext(resetsAt: string | null, extraSubtext?: string): string | null {
  const parts: string[] = []
  if (extraSubtext) parts.push(extraSubtext)
  if (resetsAt) {
    try {
      const reset = new Date(resetsAt)
      if (!Number.isNaN(reset.getTime())) {
        parts.push(`Resets ${reset.toLocaleString()}`)
      }
    } catch {
      // Ignore malformed dates from the backend.
    }
  }
  return parts.length > 0 ? parts.join(' · ') : null
}
