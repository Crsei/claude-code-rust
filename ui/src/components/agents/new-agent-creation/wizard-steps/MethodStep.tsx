import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'

/**
 * Lite-native port of upstream's `wizard-steps/MethodStep.tsx`. Pick
 * between auto-generating the agent from a description or filling it
 * in manually.
 */

type Method = 'generate' | 'manual'

const OPTIONS: Array<{ value: Method; label: string; description: string }> = [
  {
    value: 'generate',
    label: 'Generate from description',
    description: 'Let Claude draft the name + system prompt for you.',
  },
  {
    value: 'manual',
    label: 'Fill in manually',
    description: 'Enter each field yourself.',
  },
]

type Props = {
  value: Method
  onSelect: (method: Method) => void
  onCancel: () => void
}

export function MethodStep({ value, onSelect, onCancel }: Props) {
  const [focus, setFocus] = useState(() =>
    Math.max(0, OPTIONS.findIndex(o => o.value === value)),
  )

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'up') setFocus(idx => Math.max(0, idx - 1))
    else if (name === 'down') setFocus(idx => Math.min(OPTIONS.length - 1, idx + 1))
    else if (name === 'return' || name === 'enter') {
      onSelect(OPTIONS[focus]!.value)
    }
  })

  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>How would you like to create this agent?</text></strong>
      {OPTIONS.map((opt, i) => {
        const isFocused = i === focus
        return (
          <box key={opt.value} flexDirection="column">
            <box flexDirection="row" gap={1}>
              <text fg={isFocused ? c.accent : c.dim}>
                {isFocused ? '\u276F' : ' '}
              </text>
              {isFocused ? (
                <strong><text fg={c.textBright}>{opt.label}</text></strong>
              ) : (
                <text>{opt.label}</text>
              )}
            </box>
            <box paddingLeft={3}>
              <text fg={c.dim}>{opt.description}</text>
            </box>
          </box>
        )
      })}
    </box>
  )
}
