import React from 'react'
import { c } from '../../../../theme.js'
import { ModelSelector } from '../../ModelSelector.js'

/**
 * Lite-native port of upstream's `wizard-steps/ModelStep.tsx`. Thin
 * wrapper around `ModelSelector`.
 */

type Props = {
  value?: string
  onSubmit: (model?: string) => void
  onCancel: () => void
}

export function ModelStep({ value, onSubmit, onCancel }: Props) {
  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Model</text></strong>
      <ModelSelector
        initialModel={value}
        onComplete={onSubmit}
        onCancel={onCancel}
      />
    </box>
  )
}
