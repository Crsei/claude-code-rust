import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TrustDialog/TrustDialog.tsx`.
 *
 * Shown once per workspace (the "Is this a folder you trust?" gate).
 * Upstream reaches deeply into config / MCP / permissions / keybindings
 * / graceful-shutdown helpers; the Rust port surfaces those as
 * injected callbacks so the dialog stays portable:
 *
 *  - `cwd`, `trustAccepted`, and the `has*` flags all come from the
 *    caller (the Rust daemon knows how to compute them).
 *  - `onAccept` / `onExit` are the only outcomes — upstream persisted
 *    the decision through `saveCurrentProjectConfig`; that work happens
 *    on the daemon side after this component resolves.
 */

export type TrustDialogSignals = {
  hasMcpServers: boolean
  hasHooks: boolean
  hasBashExecution: boolean
  hasApiKeyHelper: boolean
  hasAwsCommands: boolean
  hasGcpCommands: boolean
  hasOtelHeadersHelper: boolean
  hasDangerousEnvVars: boolean
}

type Props = {
  cwd: string
  /** Already-accepted trust (computed by the daemon). When true the dialog
   *  self-dismisses on mount by calling `onAccept` and rendering nothing. */
  trustAccepted: boolean
  onAccept: () => void
  onExit: () => void
  signals?: TrustDialogSignals
  /** Optional analytics hook — upstream fires `tengu_trust_dialog_shown`
   *  / `tengu_trust_dialog_accept` events. */
  onShown?: (signals: TrustDialogSignals) => void
  onAccepted?: (signals: TrustDialogSignals) => void
}

const OPTIONS = [
  { label: 'Yes, I trust this folder', value: 'enable_all' as const },
  { label: 'No, exit', value: 'exit' as const },
]

const EMPTY_SIGNALS: TrustDialogSignals = {
  hasMcpServers: false,
  hasHooks: false,
  hasBashExecution: false,
  hasApiKeyHelper: false,
  hasAwsCommands: false,
  hasGcpCommands: false,
  hasOtelHeadersHelper: false,
  hasDangerousEnvVars: false,
}

export function TrustDialog({
  cwd,
  trustAccepted,
  onAccept,
  onExit,
  signals = EMPTY_SIGNALS,
  onShown,
  onAccepted,
}: Props): React.ReactElement | null {
  const [focusIndex, setFocusIndex] = useState(0)

  useEffect(() => {
    if (trustAccepted) {
      onAccept()
    } else {
      onShown?.(signals)
    }
    // biome-ignore lint/correctness/useExhaustiveDependencies: fire once
    // on mount, mirroring upstream `useEffect(..., [])`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (trustAccepted) return
    const name = event.name
    if (name === 'escape') {
      onExit()
      return
    }
    if (name === 'up') {
      setFocusIndex(i => (i - 1 + OPTIONS.length) % OPTIONS.length)
      return
    }
    if (name === 'down' || name === 'tab') {
      setFocusIndex(i => (i + 1) % OPTIONS.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      const choice = OPTIONS[focusIndex]
      if (!choice) return
      if (choice.value === 'enable_all') {
        onAccepted?.(signals)
        onAccept()
        return
      }
      onExit()
    }
  })

  if (trustAccepted) return null

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
      title="Accessing workspace:"
      titleAlignment="center"
    >
      <box flexDirection="column" paddingTop={1}>
        <text>
          <strong>{cwd}</strong>
        </text>

        <box marginTop={1}>
          <text>
            Quick safety check: Is this a project you created or one you trust?
            (Like your own code, a well-known open source project, or work
            from your team). If not, take a moment to review what&apos;s in this
            folder first.
          </text>
        </box>
        <box marginTop={1}>
          <text>
            Claude Code&apos;ll be able to read, edit, and execute files here.
          </text>
        </box>

        <box marginTop={1}>
          <text fg={c.dim}>
            Security guide: https://code.claude.com/docs/en/security
          </text>
        </box>

        <box marginTop={1} flexDirection="column">
          {OPTIONS.map((opt, i) => {
            const isFocused = i === focusIndex
            return (
              <text
                key={opt.value}
                fg={isFocused ? c.bg : c.text}
                bg={isFocused ? c.textBright : undefined}
              >
                {isFocused ? '\u25B8 ' : '  '}
                {opt.label}
              </text>
            )
          })}
        </box>

        <box marginTop={1}>
          <text fg={c.dim}>
            <em>Enter to confirm \u00b7 Esc to cancel</em>
          </text>
        </box>
      </box>
    </box>
  )
}
