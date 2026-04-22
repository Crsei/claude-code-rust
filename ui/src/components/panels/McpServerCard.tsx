import React from 'react'
import type { McpServerStatusInfo } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import { stateColor } from './state-colors.js'

/**
 * Richer rendering for a single MCP server, shown inside
 * `SubsystemStatus`.
 *
 * Lite-native counterpart of the sample tree's `MCPListPanel`
 * per-server row (`ui/examples/upstream-patterns/src/components/mcp/MCPListPanel.tsx`).
 * The upstream version supports editing, scopes, and a full detail
 * drill-down. We don't have scope information in the current protocol,
 * so this card focuses on the fields we already forward:
 *
 * - `server_info.name` / `server_info.version` when the server
 *   completes the MCP handshake — surfaced on a second line.
 * - `instructions` — when provided, shown as a dimmed caption.
 * - `transport` plus capability counts (`tools`, `resources`) on the
 *   header line so operators can spot a silently-empty server.
 * - `error` — highlighted on its own line so errors don't get lost in
 *   a single-line run of text.
 */

type Props = {
  server: McpServerStatusInfo
}

export function McpServerCard({ server }: Props) {
  const color = stateColor(server.state)
  const capabilities =
    `${server.tools_count} tool${server.tools_count === 1 ? '' : 's'}` +
    `, ${server.resources_count} resource${server.resources_count === 1 ? '' : 's'}`

  return (
    <box flexDirection="column" marginBottom={0}>
      <text>
        {'  '}
        <span fg={color}>{server.state}</span>
        {' '}
        <strong><span fg="#CDD6F4">{server.name}</span></strong>
        <span fg={c.dim}> [{server.transport}]</span>
        <span fg={c.dim}> · {capabilities}</span>
      </text>
      {server.server_info && (
        <text>
          {'    '}
          <span fg={c.dim}>
            {server.server_info.name} v{server.server_info.version}
          </span>
        </text>
      )}
      {server.instructions && server.instructions.trim().length > 0 && (
        <text>
          {'    '}
          <em>
            <span fg={c.dim}>{firstLineOf(server.instructions)}</span>
          </em>
        </text>
      )}
      {server.error && (
        <text>
          {'    '}
          <span fg="#F38BA8">{server.error}</span>
        </text>
      )}
    </box>
  )
}

function firstLineOf(text: string): string {
  const trimmed = text.trim()
  const newlineIdx = trimmed.indexOf('\n')
  if (newlineIdx === -1) return trimmed
  return trimmed.slice(0, newlineIdx)
}
