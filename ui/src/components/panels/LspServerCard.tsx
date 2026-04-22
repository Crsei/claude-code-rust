import React from 'react'
import type { LspServerInfo } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import { stateColor } from './state-colors.js'

/**
 * Richer rendering for a single LSP server, shown inside
 * `SubsystemStatus`.
 *
 * Lite-native counterpart of the sample tree's `DiagnosticsDisplay`
 * (`ui/examples/upstream-patterns/src/components/DiagnosticsDisplay.tsx`)
 * header row. Diagnostic payloads are not yet forwarded over IPC, so
 * this card stays focused on the fields that *are* forwarded:
 * `language_id`, `state`, `extensions`, `open_files_count`, `error`.
 *
 * Extension list is trimmed to the first four entries with an overflow
 * marker so a language with dozens of registered extensions doesn't
 * overflow the status box.
 */

const MAX_EXTENSIONS_SHOWN = 4

type Props = {
  server: LspServerInfo
}

function renderExtensions(extensions: string[]): string | null {
  if (!extensions || extensions.length === 0) return null
  const shown = extensions.slice(0, MAX_EXTENSIONS_SHOWN)
  const suffix =
    extensions.length > MAX_EXTENSIONS_SHOWN
      ? ` +${extensions.length - MAX_EXTENSIONS_SHOWN}`
      : ''
  return shown.join(', ') + suffix
}

export function LspServerCard({ server }: Props) {
  const color = stateColor(server.state)
  const extensions = renderExtensions(server.extensions)

  return (
    <box flexDirection="column">
      <text>
        {'  '}
        <span fg={color}>{server.state}</span>
        {' '}
        <strong><span fg="#CDD6F4">{server.language_id}</span></strong>
        <span fg={c.dim}>
          {' '}({server.open_files_count} file{server.open_files_count === 1 ? '' : 's'})
        </span>
        {extensions && (
          <span fg={c.dim}> · {extensions}</span>
        )}
      </text>
      {server.error && (
        <text>
          {'    '}
          <span fg="#F38BA8">{server.error}</span>
        </text>
      )}
    </box>
  )
}
