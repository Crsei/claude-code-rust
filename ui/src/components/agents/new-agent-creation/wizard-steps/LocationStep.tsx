import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'
import type { AgentSource } from '../../types.js'
import { locationLabelForSource } from '../../agentFileUtils.js'

/**
 * Lite-native port of upstream's
 * `wizard-steps/LocationStep.tsx`. Asks whether the new agent should
 * live under project (`.claude/agents/`) or user (`~/.claude/agents/`)
 * scope. Upstream exposes more sources (policy / local / flag) — the
 * Lite wizard only writes to user or project.
 */

type Option = {
  value: AgentSource
  label: string
  description: string
}

const OPTIONS: Option[] = [
  {
    value: 'projectSettings',
    label: 'Project',
    description: '.claude/agents/ (checked into the repo)',
  },
  {
    value: 'userSettings',
    label: 'User',
    description: '~/.claude/agents/ (available across every project)',
  },
]

type Props = {
  value: AgentSource
  onSelect: (source: AgentSource) => void
  onCancel: () => void
}

export function LocationStep({ value, onSelect, onCancel }: Props) {
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
      <strong><text fg={c.accent}>Where should this agent live?</text></strong>
      <box flexDirection="column">
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
                <text fg={c.dim}>— {locationLabelForSource(opt.value)}</text>
              </box>
              <box paddingLeft={3}>
                <text fg={c.dim}>{opt.description}</text>
              </box>
            </box>
          )
        })}
      </box>
    </box>
  )
}
