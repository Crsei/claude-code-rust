import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { McpToolInfo } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import type { ServerInfo } from './types.js'
import { plural } from './utils.js'

/**
 * Scrollable list of tools registered by a single MCP server. Enter
 * opens a detail view, Esc returns to the server menu.
 */

type Props = {
  server: ServerInfo
  tools: McpToolInfo[]
  onSelect: (tool: McpToolInfo, index: number) => void
  onBack: () => void
}

export function MCPToolListView({ server, tools, onSelect, onBack }: Props) {
  const [cursor, setCursor] = useState(0)
  const safeCursor = tools.length === 0 ? 0 : Math.max(0, Math.min(cursor, tools.length - 1))

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'escape') {
      onBack()
      return
    }
    if (tools.length === 0) return
    if (event.name === 'up' || event.name === 'k') {
      setCursor(prev => (prev === 0 ? tools.length - 1 : prev - 1))
      return
    }
    if (event.name === 'down' || event.name === 'j' || event.name === 'tab') {
      setCursor(prev => (prev + 1) % tools.length)
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const tool = tools[safeCursor]
      if (tool) onSelect(tool, safeCursor)
    }
  })

  return (
    <box flexDirection="column">
      <box paddingX={1} marginBottom={1}>
        <text>
          <strong>{`Tools for ${server.name}`}</strong>
          <span fg={c.dim}>{`  ${tools.length} ${plural(tools.length, 'tool')}`}</span>
        </text>
      </box>

      {tools.length === 0 ? (
        <box paddingX={2}>
          <text>
            <span fg={c.dim}>
              No tools discovered yet. The server may still be initializing.
            </span>
          </text>
        </box>
      ) : (
        <box flexDirection="column" paddingX={1}>
          {tools.map((tool, i) => {
            const isSelected = i === safeCursor
            const displayName = stripServerPrefix(tool.name, server.name)
            return (
              <box key={`${tool.name}-${i}`}>
                <text>
                  <span fg={isSelected ? c.accent : c.dim}>
                    {isSelected ? '▸ ' : '  '}
                  </span>
                  <span fg={isSelected ? c.textBright : c.text}>{displayName}</span>
                  {tool.description && (
                    <span fg={c.dim}>{`  — ${truncate(tool.description, 60)}`}</span>
                  )}
                </text>
              </box>
            )
          })}
        </box>
      )}

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

function stripServerPrefix(toolName: string, serverName: string): string {
  // Upstream convention: tools are exposed as `mcp__<server>__<name>`. If
  // the backend ships the bare name (common for cc-rust) we just return it.
  const prefix = `mcp__${serverName}__`
  return toolName.startsWith(prefix) ? toolName.slice(prefix.length) : toolName
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s
  return s.slice(0, max - 1) + '…'
}
