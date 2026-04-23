import React from 'react'
import { c } from '../../theme.js'
import { truncate } from '../../utils.js'
import { stringWidth } from '../string-width.js'
import { Clawd } from './Clawd.js'
import { AnimatedClawd } from './AnimatedClawd.js'
import { GuestPassesUpsell } from './GuestPassesUpsell.js'
import { OverageCreditUpsell } from './OverageCreditUpsell.js'

/**
 * Compact header shown when there is nothing new to surface (no release
 * notes, no onboarding): a Clawd + three-line info block.
 *
 * OpenTUI-native port of the upstream `LogoV2/CondensedLogo`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/CondensedLogo.tsx`).
 * Upstream read the full terminal size, main-loop model, billing type,
 * agent name, and upsell eligibility from reactive hooks. The Lite
 * port takes a `CondensedLogoData` snapshot so the Rust backend owns
 * the data flow.
 */

export type CondensedLogoData = {
  version: string
  cwd: string
  modelDisplayName: string
  billingType: string
  agentName?: string | null
  /** Available column width. Defaults to 80 for wiring tests. */
  columns?: number
  /** Opt into the animated Clawd (only safe in fullscreen terminals). */
  animated?: boolean
  appleTerminal?: boolean
  /** When set, renders the guest-passes upsell below the info block. */
  guestPasses?: { reward?: string | null }
  /** When set, renders the overage-credit upsell below the info block. */
  overageCredit?: { amount?: string | null }
}

function formatModelAndBilling(
  model: string,
  billing: string,
  textWidth: number,
): { shouldSplit: boolean; truncatedModel: string; truncatedBilling: string } {
  const combined = `${model} \u00B7 ${billing}`
  if (combined.length <= textWidth) {
    return { shouldSplit: false, truncatedModel: model, truncatedBilling: billing }
  }
  return {
    shouldSplit: true,
    truncatedModel: truncate(model, textWidth),
    truncatedBilling: truncate(billing, textWidth),
  }
}

function truncatePath(path: string, width: number): string {
  if (path.length <= width) return path
  const keep = Math.max(3, width - 1)
  return '\u2026' + path.slice(path.length - keep)
}

export function CondensedLogo({ data }: { data: CondensedLogoData }) {
  const {
    version,
    cwd,
    modelDisplayName,
    billingType,
    agentName,
    columns = 80,
    animated = false,
    appleTerminal = false,
    guestPasses,
    overageCredit,
  } = data

  const textWidth = Math.max(columns - 15, 20)
  const versionPrefix = 'Claude Code v'
  const truncatedVersion = truncate(
    version,
    Math.max(textWidth - versionPrefix.length, 6),
  )

  const { shouldSplit, truncatedModel, truncatedBilling } = formatModelAndBilling(
    modelDisplayName,
    billingType,
    textWidth,
  )

  const separator = ' \u00B7 '
  const atPrefix = '@'
  const cwdAvailableWidth = agentName
    ? textWidth - atPrefix.length - stringWidth(agentName) - separator.length
    : textWidth
  const truncatedCwd = truncatePath(cwd, Math.max(cwdAvailableWidth, 10))

  return (
    <box flexDirection="row" alignItems="center">
      {animated ? <AnimatedClawd appleTerminal={appleTerminal} /> : <Clawd appleTerminal={appleTerminal} />}
      <box flexDirection="column" marginLeft={2}>
        <text>
          <strong>Claude Code</strong>{' '}
          <span fg={c.dim}>v{truncatedVersion}</span>
        </text>
        {shouldSplit ? (
          <>
            <text fg={c.dim}>{truncatedModel}</text>
            <text fg={c.dim}>{truncatedBilling}</text>
          </>
        ) : (
          <text fg={c.dim}>
            {truncatedModel} \u00B7 {truncatedBilling}
          </text>
        )}
        <text fg={c.dim}>
          {agentName ? `@${agentName} \u00B7 ${truncatedCwd}` : truncatedCwd}
        </text>
        {guestPasses && <GuestPassesUpsell reward={guestPasses.reward} />}
        {!guestPasses && overageCredit && (
          <OverageCreditUpsell amount={overageCredit.amount} maxWidth={textWidth} twoLine />
        )}
      </box>
    </box>
  )
}
