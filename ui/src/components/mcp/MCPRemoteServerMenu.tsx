import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../../ipc/context.js'
import { useAppDispatch } from '../../store/app-store.js'
import { c } from '../../theme.js'
import { CapabilitiesSection } from './CapabilitiesSection.js'
import { StatusLine } from './MCPStdioServerMenu.js'
import type { ServerInfo } from './types.js'
import { capitalize, describeMcpConfigFilePath, isEditableScope, serverDisplayState } from './utils.js'

/**
 * Per-server actions panel for remote transports (SSE /
 * streamable-http). OAuth flows are upstream-only; cc-rust currently
 * relies on the user pre-populating bearer tokens via `headers` in
 * settings.json, so this menu exposes:
 *
 *   - View tools      (when connected + has tools)
 *   - Reconnect       (when not disabled)
 *   - Enable / Disable (writable scopes only)
 *   - Edit via /mcp   (always — points at CLI)
 *   - Remove          (writable scopes only)
 *   - Back            (always)
 */

type Props = {
  server: ServerInfo
  serverToolsCount: number
  onViewTools: () => void
  onCancel: () => void
}

type Action =
  | 'view-tools'
  | 'reconnect'
  | 'toggle-enabled'
  | 'edit-hint'
  | 'remove'
  | 'back'

export function MCPRemoteServerMenu({
  server,
  serverToolsCount,
  onViewTools,
  onCancel,
}: Props) {
  const backend = useBackend()
  const dispatch = useAppDispatch()
  const displayState = serverDisplayState(server)
  const editable = isEditableScope(server.scope)
  const headerKeys = server.config.headers
    ? Object.keys(server.config.headers)
    : []

  const options = useMemo<{ value: Action; label: string }[]>(() => {
    const out: { value: Action; label: string }[] = []
    if (displayState === 'connected' && serverToolsCount > 0) {
      out.push({ value: 'view-tools', label: 'View tools' })
    }
    if (displayState !== 'disabled') {
      out.push({ value: 'reconnect', label: 'Reconnect' })
    }
    if (editable) {
      out.push({
        value: 'toggle-enabled',
        label: displayState === 'disabled' ? 'Enable' : 'Disable',
      })
      out.push({
        value: 'edit-hint',
        label: 'Edit via /mcp edit',
      })
      out.push({ value: 'remove', label: 'Remove' })
    } else {
      out.push({
        value: 'edit-hint',
        label: 'Read-only scope — edit source instead',
      })
    }
    out.push({ value: 'back', label: 'Back' })
    return out
  }, [displayState, editable, serverToolsCount])

  const [cursor, setCursor] = useState(0)
  const safeCursor = Math.max(0, Math.min(cursor, options.length - 1))

  const handleSelect = (choice: Action) => {
    switch (choice) {
      case 'view-tools':
        onViewTools()
        return
      case 'reconnect':
        backend.send({
          type: 'mcp_command',
          command: { kind: 'reconnect_server', server_name: server.name },
        })
        return
      case 'toggle-enabled':
        backend.send({
          type: 'mcp_command',
          command: {
            kind: 'toggle_enabled',
            server_name: server.name,
            scope: server.scope,
          },
        })
        return
      case 'edit-hint':
        dispatch({
          type: 'SYSTEM_INFO',
          text: `To edit ${server.name}, run: /mcp edit ${server.name} [flags]`,
          level: 'info',
        })
        onCancel()
        return
      case 'remove':
        backend.send({
          type: 'mcp_command',
          command: {
            kind: 'remove_config',
            server_name: server.name,
            scope: server.scope,
          },
        })
        onCancel()
        return
      case 'back':
        onCancel()
        return
    }
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'escape') {
      onCancel()
      return
    }
    if (event.name === 'up' || event.name === 'k') {
      setCursor(prev => (prev === 0 ? options.length - 1 : prev - 1))
      return
    }
    if (event.name === 'down' || event.name === 'j' || event.name === 'tab') {
      setCursor(prev => (prev + 1) % options.length)
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const opt = options[safeCursor]
      if (opt) handleSelect(opt.value)
    }
  })

  return (
    <box flexDirection="column">
      <box paddingX={1} marginBottom={1}>
        <text>
          <strong>{`${capitalize(server.name)} MCP Server`}</strong>
        </text>
      </box>

      <box flexDirection="column" paddingX={1}>
        <StatusLine state={displayState} error={server.status?.error} />

        <text>
          <strong>Transport: </strong>
          <span fg={c.dim}>{server.transport}</span>
        </text>

        <text>
          <strong>URL: </strong>
          <span fg={c.dim}>{server.config.url ?? '(none)'}</span>
        </text>

        {headerKeys.length > 0 && (
          <text>
            <strong>Headers: </strong>
            <span fg={c.dim}>{headerKeys.join(', ')}</span>
          </text>
        )}

        <text>
          <strong>Config location: </strong>
          <span fg={c.dim}>{describeMcpConfigFilePath(server.scope)}</span>
        </text>

        {displayState === 'connected' && (
          <CapabilitiesSection
            serverToolsCount={serverToolsCount}
            serverResourcesCount={
              server.status?.resources_count ?? server.resources.length
            }
          />
        )}

        {serverToolsCount > 0 && (
          <text>
            <strong>Tools: </strong>
            <span fg={c.dim}>{`${serverToolsCount} tools`}</span>
          </text>
        )}
      </box>

      <box flexDirection="column" paddingX={1} marginTop={1}>
        {options.map((opt, i) => {
          const isSelected = i === safeCursor
          return (
            <box key={opt.value}>
              <text>
                <span fg={isSelected ? c.accent : c.dim}>
                  {isSelected ? '▸ ' : '  '}
                </span>
                <span fg={isSelected ? c.textBright : c.text}>{opt.label}</span>
              </text>
            </box>
          )
        })}
      </box>

      <box paddingX={1} marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>↑↓ navigate · Enter select · Esc back</span>
          </em>
        </text>
      </box>
    </box>
  )
}
