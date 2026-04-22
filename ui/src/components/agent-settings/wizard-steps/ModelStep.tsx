import React from 'react'
import { MODEL_CHOICES } from '../constants.js'
import { Select, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 7 — model override. Matches upstream `ModelStep.tsx` but sourced
 * from the shared `MODEL_CHOICES` palette so `AgentFormView` and this step
 * don't drift.
 */
export function ModelStep() {
  const { wizardData, updateWizardData, goNext, goBack } = useWizard()
  const initial = Math.max(
    0,
    MODEL_CHOICES.findIndex(opt => opt.value === (wizardData.model ?? '')),
  )
  return (
    <WizardStepLayout
      subtitle="Model override (optional)"
      footer="↑/↓ navigate · Enter select · Esc go back"
    >
      <Select
        initialIndex={initial}
        options={MODEL_CHOICES.map(opt => ({
          value: opt.value || '__inherit__',
          label: opt.label,
        }))}
        onChange={value => {
          updateWizardData({ model: value === '__inherit__' ? '' : value })
          goNext()
        }}
        onCancel={goBack}
      />
    </WizardStepLayout>
  )
}
