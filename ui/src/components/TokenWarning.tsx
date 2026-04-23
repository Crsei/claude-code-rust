import React from 'react'
import { c } from '../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TokenWarning.tsx`.
 *
 * Upstream pulls live thresholds out of `calculateTokenWarningState` and
 * the auto-compact hook; the Rust port surfaces a simpler `percentLeft`
 * and `mode` from the daemon so this component only has to format the
 * bar. The two-stage `warning` vs `error` style split mirrors the Ink
 * version — same thresholds, same copy, same dismissal affordance
 * ("Run /compact to compact & continue").
 *
 * `CONTEXT_COLLAPSE` / `REACTIVE_COMPACT` telemetry integrations are
 * intentionally absent — that machinery isn't ported to the Rust
 * frontend yet. Callers can omit `mode` and get the classic behaviour.
 */

export type TokenWarningMode = 'auto-compact' | 'reactive-only' | 'collapse'

type Props = {
  /** Tokens already used, not including the current turn in flight. */
  tokenUsage: number
  /** Model identifier — used by the caller to compute thresholds. */
  model: string
  /** Percentage of the autocompact window still available, 0\u2013100. */
  percentLeft: number
  /** When true, render with the red `error` tone. */
  isAboveErrorThreshold: boolean
  /** When false, the component renders nothing. */
  isAboveWarningThreshold: boolean
  /** Auto-compact flavour currently in effect. Defaults to `auto-compact`. */
  mode?: TokenWarningMode
  /** Extra hint appended after the main label (e.g. "Upgrade to get 500k
   *  context"). Upstream calls this `upgradeMessage`. */
  upgradeMessage?: string | null
  /** When true, the caller's compaction flow has explicitly silenced the
   *  warning (e.g. compaction is running). */
  suppressWarning?: boolean
  /** Collapse-mode status label, passed in by the caller's context-collapse
   *  state machine. Only consulted when `mode === 'collapse'`. */
  collapseLabel?: string | null
  /** When true, the component yields to the caller's auto-compact UI. */
  autoCompactEnabled?: boolean
}

export function TokenWarning({
  percentLeft,
  isAboveErrorThreshold,
  isAboveWarningThreshold,
  mode = 'auto-compact',
  upgradeMessage,
  suppressWarning,
  collapseLabel,
  autoCompactEnabled = true,
}: Props): React.ReactElement | null {
  if (!isAboveWarningThreshold || suppressWarning) return null

  if (mode === 'collapse') {
    if (collapseLabel == null) return null
    const text = upgradeMessage
      ? `${collapseLabel} \u00b7 ${upgradeMessage}`
      : collapseLabel
    return (
      <box flexDirection="row">
        <text fg={c.dim}>{text}</text>
      </box>
    )
  }

  const autocompactLabel =
    mode === 'reactive-only'
      ? `${100 - percentLeft}% context used`
      : `${percentLeft}% until auto-compact`

  if (autoCompactEnabled) {
    const text = upgradeMessage
      ? `${autocompactLabel} \u00b7 ${upgradeMessage}`
      : autocompactLabel
    return (
      <box flexDirection="row">
        <text fg={c.dim}>{text}</text>
      </box>
    )
  }

  const lowLabel = upgradeMessage
    ? `Context low (${percentLeft}% remaining) \u00b7 ${upgradeMessage}`
    : `Context low (${percentLeft}% remaining) \u00b7 Run /compact to compact & continue`

  return (
    <box flexDirection="row">
      <text fg={isAboveErrorThreshold ? c.error : c.warning}>{lowLabel}</text>
    </box>
  )
}
