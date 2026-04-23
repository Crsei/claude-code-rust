import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../ipc/context.js'
import { c } from '../theme.js'
import { MCPServerDialogCopy } from './MCPServerDialogCopy.js'

/**
 * OpenTUI port of upstream `MCPServerMultiselectDialog`
 * (`ui/examples/upstream-patterns/src/components/MCPServerMultiselectDialog.tsx`).
 *
 * Shown when several newly-seen `.mcp.json` servers arrive at once.
 * Space toggles each server, Enter commits, Esc rejects all. For every
 * approved server we issue a `toggle_enabled` IPC command to clear the
 * per-entry `disabled` flag on the backend — same shape used by the
 * `/mcp` panel's enable/disable button.
 */

type Props = {
  serverNames: string[]
  onDone: () => void
}

export function MCPServerMultiselectDialog({ serverNames, onDone }: Props) {
  const backend = useBackend()
  const [selected, setSelected] = useState<Set<string>>(() => new Set(serverNames))
  const [cursor, setCursor] = useState(0)

  const toggle = (name: string) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }

  const commit = () => {
    for (const name of serverNames) {
      if (selected.has(name)) {
        backend.send({
          type: 'mcp_command',
          command: { kind: 'toggle_enabled', server_name: name },
        })
      }
    }
    onDone()
  }

  const rejectAll = () => {
    // No backend round-trip today — entries default to disabled until
    // explicitly enabled. When a structured "deny list" command lands we
    // can forward each rejected name through it.
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
        rejectAll()
        return
      case 'return':
      case 'enter':
        commit()
        return
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
      title={`${serverNames.length} new MCP servers found in .mcp.json`}
      titleAlignment="center"
    >
      <MCPServerDialogCopy />
      <box marginTop={1}>
        <text>Select any you wish to enable.</text>
      </box>
      <box flexDirection="column" marginTop={1}>
        {serverNames.map((name, i) => {
          const isSelected = selected.has(name)
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
            </text>
          )
        })}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          <em>Space toggle · Enter confirm · Esc reject all</em>
        </text>
      </box>
    </box>
  )
}
