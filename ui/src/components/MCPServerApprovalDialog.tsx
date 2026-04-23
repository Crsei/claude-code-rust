import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../ipc/context.js'
import { c } from '../theme.js'
import { MCPServerDialogCopy } from './MCPServerDialogCopy.js'

/**
 * OpenTUI port of upstream `MCPServerApprovalDialog`
 * (`ui/examples/upstream-patterns/src/components/MCPServerApprovalDialog.tsx`).
 *
 * Prompt shown the first time a new `.mcp.json` server is discovered.
 * Choices drive the backend `toggle_enabled` command so the server's
 * `disabled` flag lands on disk at project scope.
 *
 * Upstream persists `enabledMcpjsonServers` / `disabledMcpjsonServers`
 * lists in settings; cc-rust folds both states into the per-entry
 * `disabled` bit, so we map:
 *  - `yes` / `yes_all` → enable this server (clear `disabled`).
 *  - `no`              → disable this server.
 *  - `yes_all`         → emits a follow-up `enable_all_project_mcp`
 *    hint via `onAutoApproveAll` so the caller can persist that
 *    preference once the backend grows a dedicated command.
 */

type Choice = 'yes_all' | 'yes' | 'no'

type Props = {
  serverName: string
  onDone: () => void
  onAutoApproveAll?: () => void
}

const OPTIONS: Array<{ value: Choice; label: string }> = [
  { value: 'yes_all', label: 'Use this and all future MCP servers in this project' },
  { value: 'yes', label: 'Use this MCP server' },
  { value: 'no', label: 'Continue without using this MCP server' },
]

export function MCPServerApprovalDialog({
  serverName,
  onDone,
  onAutoApproveAll,
}: Props) {
  const backend = useBackend()
  const [index, setIndex] = useState(0)

  const commit = (choice: Choice) => {
    if (choice === 'yes' || choice === 'yes_all') {
      backend.send({
        type: 'mcp_command',
        command: { kind: 'toggle_enabled', server_name: serverName },
      })
      if (choice === 'yes_all') {
        onAutoApproveAll?.()
      }
    }
    // `no` deliberately sends nothing — entries default to `disabled: false`
    // only once explicitly approved. Upstream also tracks an explicit
    // deny list; when the backend exposes one we can surface it here.
    onDone()
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    switch (event.name) {
      case 'up':
        setIndex(i => (i - 1 + OPTIONS.length) % OPTIONS.length)
        return
      case 'down':
      case 'tab':
        setIndex(i => (i + 1) % OPTIONS.length)
        return
      case 'escape':
        commit('no')
        return
      case 'return':
      case 'enter': {
        const opt = OPTIONS[index]
        if (opt) commit(opt.value)
      }
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
      title={`New MCP server found in .mcp.json: ${serverName}`}
      titleAlignment="center"
    >
      <MCPServerDialogCopy />
      <box flexDirection="column" marginTop={1}>
        {OPTIONS.map((opt, i) => {
          const selected = i === index
          return (
            <text key={opt.value}>
              <span fg={selected ? c.accent : c.dim}>
                {selected ? '\u25B8 ' : '  '}
              </span>
              <span fg={selected ? c.textBright : c.text}>{opt.label}</span>
            </text>
          )
        })}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          <em>↑↓ navigate · Enter confirm · Esc reject</em>
        </text>
      </box>
    </box>
  )
}
