import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Remote-control bridge overlay.
 *
 * OpenTUI-native port of the upstream `BridgeDialog`
 * (`ui/examples/upstream-patterns/src/components/BridgeDialog.tsx`).
 * Upstream read live state from `AppState` (replBridge* slice) and used
 * Ink's `Dialog` chrome. The Lite port keeps the same informational
 * surface but expects the caller to pass a `BridgeStatus` snapshot —
 * OpenTUI does not yet have a persistent bridge state slice, so keeping
 * this component props-driven lets the backend decide what to show.
 *
 * Interactions:
 *   - `d`                       \u2192 disconnect (calls `onDisconnect`)
 *   - `space`                   \u2192 toggle QR view
 *   - `enter` / `escape`        \u2192 close
 */

export type BridgeStatus = {
  connected: boolean
  sessionActive: boolean
  reconnecting: boolean
  error?: string | null
  connectUrl?: string | null
  sessionUrl?: string | null
  repoName?: string | null
  branchName?: string | null
  environmentId?: string | null
  sessionId?: string | null
  verbose?: boolean
}

type Props = {
  status: BridgeStatus
  onClose: () => void
  onDisconnect: () => void
  /** Optional resolver for the QR render — the OpenTUI port does not
   *  bundle `qrcode`, so callers pass the rendered lines (or null). */
  qrLines?: string[] | null
  onToggleQr?: (visible: boolean) => void
}

function statusLabelFor(s: BridgeStatus): { label: string; color: string } {
  if (s.error) return { label: 'Failed', color: c.error }
  if (s.reconnecting) return { label: 'Reconnecting\u2026', color: c.warning }
  if (s.sessionActive) return { label: 'Connected', color: c.success }
  if (s.connected) return { label: 'Ready', color: c.info }
  return { label: 'Idle', color: c.dim }
}

export function BridgeDialog({
  status,
  onClose,
  onDisconnect,
  qrLines,
  onToggleQr,
}: Props) {
  const [showQR, setShowQR] = useState(false)

  useEffect(() => {
    onToggleQr?.(showQR)
  }, [showQR, onToggleQr])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = event.sequence ?? name ?? ''
    if (input === 'd') {
      onDisconnect()
      return
    }
    if (name === 'space' || input === ' ') {
      setShowQR(prev => !prev)
      return
    }
    if (name === 'escape' || name === 'return' || name === 'enter') {
      onClose()
    }
  })

  const { label, color } = statusLabelFor(status)
  const indicator = status.error ? '\u2716' : '\u25CF'
  const contextParts: string[] = []
  if (status.repoName) contextParts.push(status.repoName)
  if (status.branchName) contextParts.push(status.branchName)
  const contextSuffix = contextParts.length
    ? ' \u00B7 ' + contextParts.join(' \u00B7 ')
    : ''

  const displayUrl = status.sessionActive ? status.sessionUrl : status.connectUrl

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={color}
      title="Remote Control"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        <span fg={color}>
          {indicator} {label}
        </span>
        <span fg={c.dim}>{contextSuffix}</span>
      </text>
      {status.error && (
        <box marginTop={1}>
          <text fg={c.error}>{status.error}</text>
        </box>
      )}
      {status.verbose && status.environmentId && (
        <text fg={c.dim}>Environment: {status.environmentId}</text>
      )}
      {status.verbose && status.sessionId && (
        <text fg={c.dim}>Session: {status.sessionId}</text>
      )}
      {displayUrl && (
        <box marginTop={1} flexDirection="column">
          <text fg={c.dim}>URL:</text>
          <text fg={c.info}>{displayUrl}</text>
        </box>
      )}
      {showQR && qrLines && qrLines.length > 0 && (
        <box marginTop={1} flexDirection="column">
          {qrLines.map((line, i) => (
            <text key={i}>{line}</text>
          ))}
        </box>
      )}
      <box marginTop={1}>
        <text fg={c.dim}>
          d to disconnect \u00B7 space for QR code \u00B7 Enter/Esc to close
        </text>
      </box>
    </box>
  )
}
