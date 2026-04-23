import React from 'react'
import { c } from '../../../../theme.js'
import { ColorPicker, type AgentColorName } from '../../ColorPicker.js'

/**
 * Lite-native port of upstream's `wizard-steps/ColorStep.tsx`. Wraps
 * the shared `ColorPicker` so the wizard owns the title + help text
 * and the picker manages the key state.
 */

type Props = {
  agentName: string
  value?: string
  onSubmit: (color?: AgentColorName) => void
  onCancel: () => void
}

export function ColorStep({ agentName, value, onSubmit, onCancel }: Props) {
  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Color</text></strong>
      <text fg={c.dim}>
        Used when this agent is mentioned in @-completions and the transcript.
      </text>
      <ColorPicker
        agentName={agentName}
        currentColor={(value as AgentColorName | undefined) ?? 'automatic'}
        onConfirm={onSubmit}
        onCancel={onCancel}
      />
    </box>
  )
}
