import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'

/**
 * Lite-native port of upstream's `wizard-steps/DescriptionStep.tsx`.
 * The description tells the orchestrator when to delegate to this
 * agent. 10+ chars is recommended.
 */

type Props = {
  value: string
  onSubmit: (description: string) => void
  onCancel: () => void
}

export function DescriptionStep({ value, onSubmit, onCancel }: Props) {
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
      if (input.trim().length > 0) onSubmit(input.trim())
      return
    }
    if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
      setInput(v => v + seq)
    }
  })

  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>When should Claude use this agent?</text></strong>
      <text fg={c.dim}>
        One or two sentences describing the trigger conditions.
      </text>
      <box flexDirection="row" gap={1}>
        <text fg={c.accent}>{'\u276F'}</text>
        <text>{input || ' '}</text>
        <text fg={c.accent}>{'\u2588'}</text>
      </box>
      {input.length < 10 && input.length > 0 && (
        <text fg={c.warning}>Try to be more descriptive (at least 10 chars).</text>
      )}
    </box>
  )
}
