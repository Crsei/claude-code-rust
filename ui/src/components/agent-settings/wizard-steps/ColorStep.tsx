import React from 'react'
import { COLOR_CHOICES } from '../constants.js'
import { Select, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 8 — display color. Matches upstream `ColorStep.tsx` via the shared
 * `COLOR_CHOICES` palette.
 */
export function ColorStep() {
  const { wizardData, updateWizardData, goNext, goBack } = useWizard()
  const initial = Math.max(
    0,
    COLOR_CHOICES.findIndex(opt => opt.value === (wizardData.color ?? '')),
  )
  return (
    <WizardStepLayout
      subtitle="Display color (optional)"
      footer="↑/↓ navigate · Enter select · Esc go back"
    >
      <Select
        initialIndex={initial}
        options={COLOR_CHOICES.map(opt => ({
          value: opt.value || '__none__',
          label: opt.label,
        }))}
        onChange={value => {
          updateWizardData({ color: value === '__none__' ? '' : value })
          goNext()
        }}
        onCancel={goBack}
      />
    </WizardStepLayout>
  )
}
