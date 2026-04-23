import React, { type ReactNode } from 'react'
import { c } from '../../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/wizard/WizardNavigationFooter.tsx`.
 *
 * Bottom row of the wizard — the upstream uses Ink's `Byline` +
 * `KeyboardShortcutHint` primitives plus `useExitOnCtrlCDWithKeybindings`
 * to show an "exit" hint when the user is mid-chord. OpenTUI doesn't
 * expose those hooks, so this port keeps the rendered content minimal
 * (navigation hints) and accepts a caller-provided `pendingExitText`
 * override so the Ctrl+C-twice UX can still be surfaced where needed.
 */

type Props = {
  instructions?: ReactNode
  /** When non-empty, shown in place of `instructions`. Mirrors the
   *  upstream `exitState.pending` branch. */
  pendingExitText?: string
}

const DEFAULT_INSTRUCTIONS = (
  <text fg={c.dim}>
    <em>
      \u2191\u2193 navigate \u00b7 Enter select \u00b7 Esc go back
    </em>
  </text>
)

export function WizardNavigationFooter({
  instructions = DEFAULT_INSTRUCTIONS,
  pendingExitText,
}: Props): React.ReactElement {
  return (
    <box marginLeft={3} marginTop={1}>
      {pendingExitText ? (
        <text fg={c.dim}>{pendingExitText}</text>
      ) : (
        instructions
      )}
    </box>
  )
}
