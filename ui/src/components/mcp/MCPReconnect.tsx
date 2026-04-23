import React, { useEffect, useState } from 'react'
import { useBackend } from '../../ipc/context.js'
import { useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'
import { describeReconnectResult } from './reconnectHelpers.js'

/**
 * Inline progress panel for a one-shot reconnect from outside the main
 * dialog (e.g. triggered from a slash-command or deep link). Fires the
 * `reconnect_server` IPC on mount and watches `mcpSettings.status`
 * until the server is no longer in a pending state, then emits an
 * outcome via `onComplete`.
 */

type Props = {
  serverName: string
  onComplete: (message: string, success: boolean) => void
}

export function MCPReconnect({ serverName, onComplete }: Props) {
  const backend = useBackend()
  const { mcpSettings } = useAppState()
  const [fired, setFired] = useState(false)

  useEffect(() => {
    if (fired) return
    setFired(true)
    backend.send({
      type: 'mcp_command',
      command: { kind: 'reconnect_server', server_name: serverName },
    })
  }, [backend, fired, serverName])

  useEffect(() => {
    if (!fired) return
    const status = mcpSettings.status.find(s => s.name === serverName)
    if (!status) return
    if (status.state === 'pending' || status.state === 'connecting') return
    const outcome = describeReconnectResult(status, serverName)
    onComplete(outcome.message, outcome.success)
  }, [fired, mcpSettings.status, onComplete, serverName])

  return (
    <box flexDirection="column" paddingX={1} paddingY={1}>
      <text>
        <strong>Reconnecting to </strong>
        <span>{serverName}</span>
      </text>
      <text>
        <span fg={c.dim}>Establishing connection to MCP server…</span>
      </text>
    </box>
  )
}
