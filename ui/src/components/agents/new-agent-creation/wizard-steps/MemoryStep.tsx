import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'

/**
 * Lite-native port of upstream's `wizard-steps/MemoryStep.tsx`. The
 * three upstream memory scopes are `session`, `project`, and
 * `inherit`; project memory is the most common default.
 */

type MemoryScope = 'session' | 'project' | 'inherit'

const OPTIONS: Array<{ value: MemoryScope; label: string; description: string }> = [
  {
    value: 'inherit',
    label: 'Inherit',
    description: 'Use the same scope as the orchestrator.',
  },
  {
    value: 'session',
    label: 'Session',
    description: 'Forget everything between sessions.',
  },
  {
    value: 'project',
    label: 'Project',
    description: 'Persist learnings inside .cc-rust/ memory for this repo.',
  },
]

type Props = {
  value?: string
  onSubmit: (memory?: string) => void
  onCancel: () => void
}

export function MemoryStep({ value, onSubmit, onCancel }: Props) {
  const [focus, setFocus] = useState(() => {
    const idx = OPTIONS.findIndex(o => o.value === value)
    return idx >= 0 ? idx : 0
  })

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
      const pick = OPTIONS[focus]!.value
      onSubmit(pick === 'inherit' ? undefined : pick)
    }
  })

  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Memory scope</text></strong>
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
