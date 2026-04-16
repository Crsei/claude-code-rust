import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { useBackend } from '../ipc/context.js'
import { useAppDispatch } from '../store/app-store.js'
import type { PermissionRequest } from '../store/app-store.js'

interface Props {
  request: PermissionRequest
}

export function PermissionDialog({ request }: Props) {
  const [selected, setSelected] = useState(0)
  const backend = useBackend()
  const dispatch = useAppDispatch()

  const options = request.options.length > 0
    ? request.options
    : ['Allow', 'Deny', 'Always Allow']

  const decide = (decision: string) => {
    backend.send({
      type: 'permission_response',
      tool_use_id: request.toolUseId,
      decision: decision.toLowerCase().replace(/\s+/g, '_'),
    })
    dispatch({ type: 'PERMISSION_DISMISS' })
  }

  useKeyboard((e) => {
    if (e.eventType === 'release') return

    const input = e.sequence?.length === 1 ? e.sequence : e.name?.length === 1 ? e.name : ''

    // Quick keys
    if (input === 'y' || input === 'Y') { decide('allow'); return }
    if (input === 'n' || input === 'N') { decide('deny'); return }
    if (input === 'a' || input === 'A') { decide('always_allow'); return }
    if (e.name === 'escape') { decide('deny'); return }

    // Arrow navigation
    if (e.name === 'left' || input === 'h') {
      setSelected(s => Math.max(0, s - 1))
      return
    }
    if (e.name === 'right' || input === 'l') {
      setSelected(s => Math.min(options.length - 1, s + 1))
      return
    }
    if (e.name === 'tab') {
      setSelected(s => (s + 1) % options.length)
      return
    }

    // Enter to confirm selection
    if (e.name === 'return' || e.name === 'enter') {
      const opt = options[selected]
      decide(opt.toLowerCase().replace(/\s+/g, '_'))
    }
  })

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title="Permission Required"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text><strong><span fg={c.warning}>Permission Required</span></strong></text>
      <box marginTop={1} flexDirection="column">
        <text>
          <span fg={c.dim}>Tool: </span>
          <strong>{request.tool}</strong>
        </text>
        {request.command && (
          <text>
            <span fg={c.dim}>Command: </span>
            {request.command}
          </text>
        )}
      </box>
      <box marginTop={1} gap={2}>
        {options.map((opt, i) => {
          const isSelected = i === selected
          const shortcut = opt === 'Allow' ? 'y'
            : opt === 'Deny' ? 'n'
            : opt === 'Always Allow' ? 'a'
            : null

          return (
            <box key={opt}>
              <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                <strong>{` ${opt} `}</strong>
              </text>
              {shortcut && <text fg={c.dim}> ({shortcut})</text>}
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text><em><span fg={c.dim}>Use arrow keys or y/n/a to decide. Esc to deny.</span></em></text>
      </box>
    </box>
  )
}
