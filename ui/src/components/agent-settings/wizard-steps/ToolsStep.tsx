import React from 'react'
import { ToolSelector } from '../ToolSelector.js'
import { WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 6 — tool allow-list. Delegates to `ToolSelector`, which owns the
 * bucket-checkbox UI and the all-selected → `undefined` normalization.
 */
export function ToolsStep() {
  const { wizardData, updateWizardData, goNext, goBack } = useWizard()

  return (
    <WizardStepLayout
      subtitle="Select tools available to the agent"
      footer="↑/↓ navigate · Enter toggle · Esc go back"
    >
      <ToolSelector
        initialTools={wizardData.selectedTools}
        onComplete={tools => {
          updateWizardData({ selectedTools: tools })
          goNext()
        }}
        onCancel={goBack}
      />
    </WizardStepLayout>
  )
}
