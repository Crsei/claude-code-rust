import React, { useState } from 'react'
import { Box, Text, useInput } from 'ink-terminal'
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

  useInput((input, key) => {
    // Quick keys
    if (input === 'y' || input === 'Y') { decide('allow'); return }
    if (input === 'n' || input === 'N') { decide('deny'); return }
    if (input === 'a' || input === 'A') { decide('always_allow'); return }
    if (key.escape) { decide('deny'); return }

    // Arrow navigation
    if (key.leftArrow || input === 'h') {
      setSelected(s => Math.max(0, s - 1))
      return
    }
    if (key.rightArrow || input === 'l') {
      setSelected(s => Math.min(options.length - 1, s + 1))
      return
    }
    if (key.tab) {
      setSelected(s => (s + 1) % options.length)
      return
    }

    // Enter to confirm selection
    if (key.return) {
      const opt = options[selected]
      decide(opt.toLowerCase().replace(/\s+/g, '_'))
    }
  })

  return (
    <Box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="round"
      borderColor="ansi:yellow"
      paddingX={2}
      paddingY={1}
    >
      <Text color="ansi:yellow" bold>Permission Required</Text>
      <Box marginTop={1} flexDirection="column">
        <Text>
          <Text dim>Tool: </Text>
          <Text bold>{request.tool}</Text>
        </Text>
        {request.command && (
          <Text>
            <Text dim>Command: </Text>
            <Text>{request.command}</Text>
          </Text>
        )}
      </Box>
      <Box marginTop={1} gap={2}>
        {options.map((opt, i) => {
          const isSelected = i === selected
          const shortcut = opt === 'Allow' ? 'y'
            : opt === 'Deny' ? 'n'
            : opt === 'Always Allow' ? 'a'
            : null

          return (
            <Box key={opt}>
              <Text inverse={isSelected} bold={isSelected}>
                {` ${opt} `}
              </Text>
              {shortcut && <Text dim> ({shortcut})</Text>}
            </Box>
          )
        })}
      </Box>
      <Box marginTop={1}>
        <Text dim italic>Use arrow keys or y/n/a to decide. Esc to deny.</Text>
      </Box>
    </Box>
  )
}
