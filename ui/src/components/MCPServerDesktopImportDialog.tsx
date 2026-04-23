import React, { useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../ipc/context.js'
import { useAppState } from '../store/app-store.js'
import type {
  ConfigScope,
  McpServerConfigEntry,
} from '../ipc/protocol.js'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `MCPServerDesktopImportDialog`
 * (`ui/examples/upstream-patterns/src/components/MCPServerDesktopImportDialog.tsx`).
 *
 * Imports a set of MCP server definitions (typically the user's Claude
 * Desktop config) into the selected scope. Upstream collects the
 * existing server list up-front with `getAllMcpConfigs()`; Lite reads
 * the already-mirrored `mcpSettings.entries` slice from the store and
 * issues one `upsert_config` IPC command per accepted entry.
 *
 * Collision handling mirrors upstream: reuse the same name when the
 * entry does not already exist, otherwise append `_1`, `_2`, … until the
 * name is free. Pre-selection excludes collisions so the default Enter
 * press is safe.
 */

type Props = {
  /** New servers to offer for import. Record of name → entry (scope is
   *  assigned fresh from `scope` below when we upsert). */
  servers: Record<string, Omit<McpServerConfigEntry, 'name' | 'scope'> & Partial<Pick<McpServerConfigEntry, 'scope'>>>
  scope: ConfigScope
  onDone: () => void
}

function nextFreeName(base: string, existing: Set<string>): string {
  if (!existing.has(base)) return base
  let counter = 1
  while (existing.has(`${base}_${counter}`)) counter++
  return `${base}_${counter}`
}

export function MCPServerDesktopImportDialog({ servers, scope, onDone }: Props) {
  const backend = useBackend()
  const state = useAppState()
  const existingEntries = state.mcpSettings.entries
  const existingNames = useMemo(
    () => new Set<string>(existingEntries.map(entry => entry.name)),
    [existingEntries],
  )

  const serverNames = useMemo(() => Object.keys(servers), [servers])
  const collisions = useMemo(
    () => new Set<string>(serverNames.filter(name => existingNames.has(name))),
    [existingNames, serverNames],
  )

  const [selected, setSelected] = useState<Set<string>>(() => {
    const initial = new Set<string>()
    for (const name of serverNames) {
      if (!collisions.has(name)) initial.add(name)
    }
    return initial
  })
  const [cursor, setCursor] = useState(0)

  useEffect(() => {
    // Refresh config list so `existingNames` is authoritative.
    backend.send({ type: 'mcp_command', command: { kind: 'query_config' } })
  }, [backend])

  const toggle = (name: string) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }

  const submit = () => {
    const taken = new Set(existingNames)
    for (const name of serverNames) {
      if (!selected.has(name)) continue
      const raw = servers[name]
      if (!raw) continue
      const finalName = nextFreeName(name, taken)
      taken.add(finalName)
      const entry: McpServerConfigEntry = {
        ...raw,
        name: finalName,
        transport: raw.transport,
        scope,
      }
      backend.send({
        type: 'mcp_command',
        command: { kind: 'upsert_config', entry },
      })
    }
    onDone()
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    switch (event.name) {
      case 'up':
        setCursor(i => (i - 1 + serverNames.length) % serverNames.length)
        return
      case 'down':
      case 'tab':
        setCursor(i => (i + 1) % serverNames.length)
        return
      case 'space': {
        const name = serverNames[cursor]
        if (name) toggle(name)
        return
      }
      case 'escape':
        onDone()
        return
      case 'return':
      case 'enter':
        submit()
        return
    }
  })

  if (serverNames.length === 0) {
    return (
      <box borderStyle="rounded" borderColor={c.dim} padding={1}>
        <text>No MCP servers to import.</text>
      </box>
    )
  }

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.success}
      paddingX={2}
      paddingY={1}
      title="Import MCP Servers from Claude Desktop"
      titleAlignment="center"
    >
      <text fg={c.dim}>
        Found {serverNames.length} MCP server{serverNames.length === 1 ? '' : 's'} in Claude Desktop.
      </text>
      {collisions.size > 0 && (
        <box marginTop={1}>
          <text fg={c.warning}>
            Note: Some servers already exist with the same name. If selected, they will be imported with a numbered suffix.
          </text>
        </box>
      )}
      <box marginTop={1}>
        <text>Please select the servers you want to import:</text>
      </box>
      <box flexDirection="column" marginTop={1}>
        {serverNames.map((name, i) => {
          const isSelected = selected.has(name)
          const collides = collisions.has(name)
          const atCursor = i === cursor
          return (
            <text key={name}>
              <span fg={atCursor ? c.accent : c.dim}>
                {atCursor ? '\u25B8 ' : '  '}
              </span>
              <span fg={isSelected ? c.success : c.dim}>
                {isSelected ? '[\u2713] ' : '[ ] '}
              </span>
              <span fg={atCursor ? c.textBright : c.text}>{name}</span>
              {collides && (
                <span fg={c.warning}> (already exists)</span>
              )}
            </text>
          )
        })}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          <em>Space toggle · Enter import · Esc cancel</em>
        </text>
      </box>
    </box>
  )
}
