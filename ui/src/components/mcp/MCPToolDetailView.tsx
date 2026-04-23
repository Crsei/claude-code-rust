import React from 'react'
import { useKeyboard } from '@opentui/react'
import type { McpToolInfo } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import type { ServerInfo } from './types.js'

/**
 * Read-only detail view for a single MCP tool. cc-rust's
 * `ToolsDiscovered` event only carries `{ name, description }`, so we
 * render exactly what we have. Parameter schemas require a richer
 * protocol extension and are deferred — see upstream
 * `MCPToolDetailView` for the full shape we'll eventually match.
 */

type Props = {
  tool: McpToolInfo
  server: ServerInfo
  onBack: () => void
}

export function MCPToolDetailView({ tool, server, onBack }: Props) {
  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'escape' || event.name === 'return' || event.name === 'enter') {
      onBack()
    }
  })

  const displayName = stripServerPrefix(tool.name, server.name)

  return (
    <box flexDirection="column">
      <box paddingX={1} marginBottom={1}>
        <text>
          <strong>{displayName}</strong>
          <span fg={c.dim}>{`  — ${server.name}`}</span>
        </text>
      </box>

      <box flexDirection="column" paddingX={1}>
        <text>
          <strong>Full name: </strong>
          <span fg={c.dim}>{tool.name}</span>
        </text>

        {tool.description && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>Description:</strong>
            </text>
            <text>{tool.description}</text>
          </box>
        )}

        {!tool.description && (
          <box marginTop={1}>
            <text>
              <span fg={c.dim}>No description provided by server.</span>
            </text>
          </box>
        )}
      </box>

      <box paddingX={1} marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>Esc / Enter to return to the tool list</span>
          </em>
        </text>
      </box>
    </box>
  )
}

function stripServerPrefix(toolName: string, serverName: string): string {
  const prefix = `mcp__${serverName}__`
  return toolName.startsWith(prefix) ? toolName.slice(prefix.length) : toolName
}
