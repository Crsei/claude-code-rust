import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'
import type { AgentDefinitionEntry } from '../../../../ipc/protocol.js'
import { validateAgentType } from '../../validateAgent.js'

/**
 * Lite-native port of upstream's `wizard-steps/TypeStep.tsx`. Collects
 * the agent's kebab-case name, enforces the same regex upstream uses,
 * and flags collisions against existing agents.
 */

type Props = {
  value: string
  existingAgents: AgentDefinitionEntry[]
  onSubmit: (agentType: string) => void
  onCancel: () => void
}

export function TypeStep({ value, existingAgents, onSubmit, onCancel }: Props) {
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
      if (!validateAgentType(input) && !existingAgents.some(a => a.name === input)) {
        onSubmit(input)
      }
      return
    }
    if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
      setInput(v => v + seq)
    }
  })

  const typeError = validateAgentType(input)
  const duplicate = existingAgents.some(a => a.name === input)

  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Agent name</text></strong>
      <text fg={c.dim}>
        Kebab-case, 3–50 characters. This is how the orchestrator delegates to it.
      </text>
      <box flexDirection="row" gap={1}>
        <text fg={c.accent}>{'\u276F'}</text>
        <text>{input || ' '}</text>
        <text fg={c.accent}>{'\u2588'}</text>
      </box>
      {typeError && <text fg={c.error}>{typeError}</text>}
      {!typeError && duplicate && (
        <text fg={c.error}>An agent named "{input}" already exists.</text>
      )}
      {!typeError && !duplicate && input.length > 0 && (
        <text fg={c.success}>Enter to accept this name.</text>
      )}
    </box>
  )
}
