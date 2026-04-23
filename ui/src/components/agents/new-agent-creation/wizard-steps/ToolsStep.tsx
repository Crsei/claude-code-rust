import React from 'react'
import { c } from '../../../../theme.js'
import { ToolSelector, type ToolSpec } from '../../ToolSelector.js'

/**
 * Lite-native port of upstream's `wizard-steps/ToolsStep.tsx`. Thin
 * wizard wrapper over `ToolSelector` — the multi-select list component
 * owns all the actual interaction state.
 */

type Props = {
  availableTools: ToolSpec[]
  selected: string[] | undefined
  onSubmit: (tools: string[] | undefined) => void
  onCancel: () => void
}

export function ToolsStep({ availableTools, selected, onSubmit, onCancel }: Props) {
  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Tools</text></strong>
      <text fg={c.dim}>
        Choose which tools this agent can call. "All tools" inherits the orchestrator set.
      </text>
      <ToolSelector
        tools={availableTools}
        selected={selected}
        onComplete={onSubmit}
        onCancel={onCancel}
      />
    </box>
  )
}
