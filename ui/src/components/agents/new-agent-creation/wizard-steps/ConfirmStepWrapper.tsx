import React, { type ReactNode } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'
import type { AgentDefinitionEntry } from '../../../../ipc/protocol.js'
import type { DraftAgent } from '../../types.js'
import { validateAgent } from '../../validateAgent.js'

/**
 * Lite-native port of upstream's `wizard-steps/ConfirmStepWrapper.tsx`.
 *
 * Wraps `ConfirmStep` in a keyboard-enabled shell. Upstream makes the
 * Confirm page interactive (retry generation / go back / commit); the
 * Lite version exposes the same entry points via `onConfirm` and
 * `onBack`, and blocks commit when validation fails.
 */

type Props = {
  draft: DraftAgent
  availableTools: string[]
  existingAgents: AgentDefinitionEntry[]
  onConfirm: () => void
  onBack: () => void
  children: ReactNode
}

export function ConfirmStepWrapper({
  draft,
  availableTools,
  existingAgents,
  onConfirm,
  onBack,
  children,
}: Props) {
  const validation = validateAgent(draft, availableTools, existingAgents)

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.toLowerCase()
    if (name === 'escape' || seq === 'b') {
      onBack()
      return
    }
    if (seq === 'y' || name === 'return' || name === 'enter') {
      if (validation.isValid) onConfirm()
    }
  })

  return (
    <box flexDirection="column" gap={1}>
      {children}

      {validation.errors.length > 0 && (
        <box flexDirection="column" marginTop={1}>
          {validation.errors.map((err, i) => (
            <text key={i} fg={c.error}>• {err}</text>
          ))}
        </box>
      )}
      {validation.warnings.length > 0 && (
        <box flexDirection="column">
          {validation.warnings.map((warn, i) => (
            <text key={i} fg={c.warning}>• {warn}</text>
          ))}
        </box>
      )}

      <box marginTop={1}>
        <text fg={c.dim}>
          {validation.isValid
            ? 'Enter / y to create · b to go back · Esc to cancel'
            : 'Fix errors above to create · b to go back · Esc to cancel'}
        </text>
      </box>
    </box>
  )
}
