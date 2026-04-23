import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * "Bypass Permissions mode" danger-mode opt-in dialog.
 *
 * OpenTUI-native port of the upstream `BypassPermissionsModeDialog`
 * (`ui/examples/upstream-patterns/src/components/BypassPermissionsModeDialog.tsx`).
 * Upstream called `gracefulShutdownSync` on decline and wrote
 * `skipDangerousModePermissionPrompt` into user settings on accept;
 * the Lite port hands both back through callbacks so the surrounding
 * backend/bootstrap code owns persistence and shutdown.
 */

type Choice = 'accept' | 'decline'

type Props = {
  onAccept: () => void
  /** Called when the user selects "No, exit" or presses Esc. Caller is
   *  expected to trigger a graceful shutdown. */
  onDecline: () => void
}

const OPTIONS: Array<{ label: string; value: Choice }> = [
  { label: 'No, exit', value: 'decline' },
  { label: 'Yes, I accept', value: 'accept' },
]

export function BypassPermissionsModeDialog({ onAccept, onDecline }: Props) {
  const [selected, setSelected] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const input = (seq ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (name === 'escape') {
      onDecline()
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || input === 'j' || name === 'tab') {
      setSelected(prev => Math.min(OPTIONS.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const option = OPTIONS[selected]
      if (option?.value === 'accept') onAccept()
      else onDecline()
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
      borderColor={c.error}
      title="WARNING: Claude Code running in Bypass Permissions mode"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        In Bypass Permissions mode, Claude Code will not ask for your approval
        before running potentially dangerous commands.
      </text>
      <box marginTop={1}>
        <text>
          This mode should only be used in a sandboxed container/VM that has
          restricted internet access and can easily be restored if damaged.
        </text>
      </box>
      <box marginTop={1}>
        <text>
          By proceeding, you accept all responsibility for actions taken while
          running in Bypass Permissions mode.
        </text>
      </box>
      <box marginTop={1}>
        <text fg={c.info}>
          Learn more: https://docs.claude.com/en/docs/claude-code/security
        </text>
      </box>
      <box marginTop={1} flexDirection="column">
        {OPTIONS.map((opt, i) => {
          const isSelected = i === selected
          return (
            <box key={opt.value} flexDirection="row">
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` ${opt.label} `}</strong>
              </text>
            </box>
          )
        })}
      </box>
    </box>
  )
}
