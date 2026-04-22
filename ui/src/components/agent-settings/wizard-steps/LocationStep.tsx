import React from 'react'
import { Select, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 0 — where the agent file lands on disk.
 * Mirrors upstream `LocationStep.tsx`.
 */
export function LocationStep() {
  const { goNext, updateWizardData, cancel } = useWizard()
  return (
    <WizardStepLayout
      subtitle="Choose location"
      footer="↑/↓ navigate · Enter select · Esc cancel"
    >
      <Select
        options={[
          { value: 'project', label: 'Project (.cc-rust/agents/)' },
          { value: 'user', label: 'Personal (~/.cc-rust/agents/)' },
        ]}
        onChange={value => {
          updateWizardData({ location: value as 'project' | 'user' })
          goNext()
        }}
        onCancel={cancel}
      />
    </WizardStepLayout>
  )
}
