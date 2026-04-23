import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * "Enable auto mode?" opt-in dialog.
 *
 * OpenTUI-native port of the upstream `AutoModeOptInDialog`
 * (`ui/examples/upstream-patterns/src/components/AutoModeOptInDialog.tsx`).
 * The copy here mirrors the legally-reviewed upstream text byte-for-byte.
 * Persistence (`skipAutoPermissionPrompt`, default-mode preference) and
 * analytics events live in the Rust backend — the frontend delegates
 * that via `onAccept(setAsDefault)` / `onDecline()`.
 */

// NOTE: legally-reviewed copy — keep in sync with the upstream constant.
export const AUTO_MODE_DESCRIPTION =
  "Auto mode lets Claude handle permission prompts automatically \u2014 Claude checks each tool call for risky actions and prompt injection before executing. Actions Claude identifies as safe are executed, while actions Claude identifies as risky are blocked and Claude may try a different approach. Ideal for long-running tasks. Sessions are slightly more expensive. Claude can make mistakes that allow harmful commands to run, it's recommended to only use in isolated environments. Shift+Tab to change mode."

type Choice = 'accept' | 'accept-default' | 'decline'

type Props = {
  onAccept: (setAsDefault: boolean) => void
  onDecline: () => void
  /** When true, the "No" button label becomes "No, exit" to reflect
   *  the startup-gate flow. */
  declineExits?: boolean
}

export function AutoModeOptInDialog({
  onAccept,
  onDecline,
  declineExits = false,
}: Props) {
  const options: Array<{ label: string; value: Choice }> = [
    { label: 'Yes, and make it my default mode', value: 'accept-default' },
    { label: 'Yes, enable auto mode', value: 'accept' },
    { label: declineExits ? 'No, exit' : 'No, go back', value: 'decline' },
  ]

  const [selected, setSelected] = useState(0)

  const commit = (choice: Choice) => {
    if (choice === 'decline') {
      onDecline()
    } else {
      onAccept(choice === 'accept-default')
    }
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'escape') {
      onDecline()
      return
    }
    if (name === 'up' || event.sequence === 'k') {
      setSelected(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || event.sequence === 'j' || name === 'tab') {
      setSelected(prev => Math.min(options.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const option = options[selected]
      if (option) commit(option.value)
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
      title="Enable auto mode?"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>{AUTO_MODE_DESCRIPTION}</text>
      <box marginTop={1}>
        <text fg={c.info}>
          Learn more: https://docs.claude.com/en/docs/claude-code/security
        </text>
      </box>
      <box marginTop={1} flexDirection="column">
        {options.map((opt, i) => {
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
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>Up/Down to move \u00B7 Enter to confirm \u00B7 Esc to decline</span>
          </em>
        </text>
      </box>
    </box>
  )
}
