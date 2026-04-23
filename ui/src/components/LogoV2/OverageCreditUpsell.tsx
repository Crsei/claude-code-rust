import React from 'react'
import { c } from '../../theme.js'
import { truncate } from '../../utils.js'

/**
 * Condensed upsell row advertising the overage-credit grant.
 *
 * OpenTUI-native port of the upstream `LogoV2/OverageCreditUpsell`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/OverageCreditUpsell.tsx`).
 * Upstream read the grant from a cached `/overage_credit_grant`
 * response and tracked impressions in global config. The Lite port
 * receives `amount` as a prop; eligibility, impression caps, and
 * background refresh are Rust-side concerns.
 */

const FEED_SUBTITLE = 'On us. Works on third-party apps \u00B7 /extra-usage'

type Props = {
  /** Formatted amount (e.g. "$5"). If null / empty, nothing renders. */
  amount?: string | null
  maxWidth?: number
  /** When true, render as a two-line title + subtitle block. */
  twoLine?: boolean
}

function getFeedTitle(amount: string): string {
  return `${amount} in extra usage`
}

function getUsageText(amount: string): string {
  return `${amount} in extra usage for third-party apps \u00B7 /extra-usage`
}

export function OverageCreditUpsell({ amount, maxWidth, twoLine }: Props) {
  if (!amount) return null
  if (twoLine) {
    const title = getFeedTitle(amount)
    return (
      <>
        <text fg={c.accent}>{maxWidth ? truncate(title, maxWidth) : title}</text>
        <text fg={c.dim}>
          {maxWidth ? truncate(FEED_SUBTITLE, maxWidth) : FEED_SUBTITLE}
        </text>
      </>
    )
  }

  const text = getUsageText(amount)
  const display = maxWidth ? truncate(text, maxWidth) : text
  const highlightLen = Math.min(getFeedTitle(amount).length, display.length)

  return (
    <text fg={c.dim}>
      <span fg={c.accent}>{display.slice(0, highlightLen)}</span>
      {display.slice(highlightLen)}
    </text>
  )
}
