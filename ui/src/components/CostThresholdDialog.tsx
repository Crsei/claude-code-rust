import React from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * "You've spent $X on the Anthropic API this session." notice.
 *
 * OpenTUI-native port of the upstream `CostThresholdDialog`
 * (`ui/examples/upstream-patterns/src/components/CostThresholdDialog.tsx`).
 * Single-action acknowledgement \u2014 any confirming keypress resolves
 * via `onDone`.
 */

type Props = {
  onDone: () => void
  /**
   * Accumulated session cost in USD. Upstream hard-coded "$5"; this port
   * accepts it so the caller can pick the threshold (the cost-tracker
   * lives in the Rust backend). Falls back to "$5" when omitted.
   */
  amountUsd?: number
}

const DOCS_URL = 'https://docs.claude.com/en/docs/claude-code/costs'

function formatUsd(amount: number): string {
  if (amount >= 10) return `$${amount.toFixed(0)}`
  return `$${amount.toFixed(2)}`
}

export function CostThresholdDialog({ onDone, amountUsd }: Props) {
  const amountLabel = amountUsd !== undefined ? formatUsd(amountUsd) : '$5'

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (
      event.name === 'return' ||
      event.name === 'enter' ||
      event.name === 'escape' ||
      event.name === 'space'
    ) {
      onDone()
    }
  })

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title={`You've spent ${amountLabel} on the Anthropic API this session.`}
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>Learn more about how to monitor your spending:</text>
      <box marginTop={1}>
        <text fg={c.info}>{DOCS_URL}</text>
      </box>
      <box marginTop={1}>
        <text>
          <strong>
            <span fg={c.bg} bg={c.textBright}>{' Got it, thanks! '}</span>
          </strong>
        </text>
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          Press <strong>Enter</strong> to continue\u2026
        </text>
      </box>
    </box>
  )
}
