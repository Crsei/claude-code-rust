import React from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/PrBadge.tsx`.
 *
 * Renders a `PR #123` label colour-coded by review state. Upstream's
 * `Link` (OSC 8 hyperlink) is reproduced with the same escape sequence
 * used by `FilePathLink` — terminals that don't support OSC 8 still get
 * the styled text.
 */

export type PrReviewState =
  | 'approved'
  | 'changes_requested'
  | 'pending'
  | 'merged'

type Props = {
  number: number
  url: string
  reviewState?: PrReviewState
  bold?: boolean
}

const COLOR_FOR_STATE: Record<PrReviewState, string> = {
  approved: c.success,
  changes_requested: c.error,
  pending: c.warning,
  merged: c.accent,
}

function osc8(url: string, text: string): string {
  return `\x1b]8;;${url}\x1b\\${text}\x1b]8;;\x1b\\`
}

export function PrBadge({ number, url, reviewState, bold }: Props) {
  const stateColor = reviewState ? COLOR_FOR_STATE[reviewState] : undefined
  const labelColor = stateColor ?? (bold ? c.text : c.dim)
  const prefixColor = bold ? c.text : c.dim

  const label = `#${number}`
  const linked = osc8(url, label)

  return (
    <text>
      <span fg={prefixColor}>PR </span>
      {bold ? (
        <strong>
          <span fg={labelColor}>{linked}</span>
        </strong>
      ) : (
        <span fg={labelColor}>{linked}</span>
      )}
    </text>
  )
}
