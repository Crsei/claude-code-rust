import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'

/**
 * Lite-native port of upstream's `wizard-steps/PromptStep.tsx`. A
 * multi-line system-prompt editor. Enter commits; Shift+Enter inserts
 * a newline to match upstream's keybinding.
 */

type Props = {
  value: string
  onSubmit: (systemPrompt: string) => void
  onCancel: () => void
}

export function PromptStep({ value, onSubmit, onCancel }: Props) {
  const [input, setInput] = useState(value)

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence

    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setInput(v => v.slice(0, -1))
      return
    }
    if (name === 'return' || name === 'enter') {
      if (event.shift) {
        setInput(v => `${v}\n`)
        return
      }
      if (input.trim().length >= 20) onSubmit(input.trim())
      return
    }
    if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
      setInput(v => v + seq)
    }
  })

  const lines = input.length > 0 ? input.split('\n') : [' ']

  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>System prompt</text></strong>
      <text fg={c.dim}>
        At least 20 characters. Shift+Enter for newline, Enter to submit.
      </text>
      <box flexDirection="column">
        {lines.map((line, i) => (
          <box key={i} flexDirection="row" gap={1}>
            <text fg={c.accent}>{i === 0 ? '\u276F' : ' '}</text>
            <text>{line || ' '}</text>
            {i === lines.length - 1 && <text fg={c.accent}>{'\u2588'}</text>}
          </box>
        ))}
      </box>
      {input.length > 0 && input.trim().length < 20 && (
        <text fg={c.warning}>System prompt is too short (minimum 20 characters).</text>
      )}
    </box>
  )
}
