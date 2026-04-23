import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Dialog shown when switching from the `latest` release channel to
 * `stable`, where the stable channel may be on an older version.
 *
 * OpenTUI-native port of the upstream `ChannelDowngradeDialog`
 * (`ui/examples/upstream-patterns/src/components/ChannelDowngradeDialog.tsx`).
 */

export type ChannelDowngradeChoice = 'downgrade' | 'stay' | 'cancel'

type Props = {
  currentVersion: string
  onChoice: (choice: ChannelDowngradeChoice) => void
}

const OPTIONS: Array<{ label: (v: string) => string; value: ChannelDowngradeChoice }> = [
  { label: () => 'Allow possible downgrade to stable version', value: 'downgrade' },
  {
    label: v => `Stay on current version (${v}) until stable catches up`,
    value: 'stay',
  },
]

export function ChannelDowngradeDialog({ currentVersion, onChoice }: Props) {
  const [selected, setSelected] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = event.sequence ?? name ?? ''
    if (name === 'escape') {
      onChoice('cancel')
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
      if (option) onChoice(option.value)
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
      title="Switch to Stable Channel"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        The stable channel may have an older version than what you&apos;re
        currently running ({currentVersion}).
      </text>
      <box marginTop={1}>
        <text fg={c.dim}>How would you like to handle this?</text>
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
                <strong>{` ${opt.label(currentVersion)} `}</strong>
              </text>
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>Up/Down to move \u00B7 Enter to confirm \u00B7 Esc to cancel</span>
          </em>
        </text>
      </box>
    </box>
  )
}
