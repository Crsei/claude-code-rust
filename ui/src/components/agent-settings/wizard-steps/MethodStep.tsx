import React from 'react'
import { Select, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 1 — "generate with Claude" vs "manual configuration". Picks the
 * destination step: `generate` → GenerateStep (2), `manual` → TypeStep (3).
 * Mirrors upstream `MethodStep.tsx`.
 */
export function MethodStep() {
  const { goNext, goBack, goToStep, updateWizardData } = useWizard()
  return (
    <WizardStepLayout
      subtitle="Creation method"
      footer="↑/↓ navigate · Enter select · Esc go back"
    >
      <Select
        options={[
          {
            value: 'generate',
            label: 'Generate with Claude (recommended)',
            description: 'Describe what the agent should do — the model writes the prompt',
          },
          {
            value: 'manual',
            label: 'Manual configuration',
            description: 'Fill every field by hand',
          },
        ]}
        onChange={value => {
          const method = value as 'generate' | 'manual'
          updateWizardData({ method, wasGenerated: method === 'generate' })
          if (method === 'generate') {
            goNext()
          } else {
            // Skip GenerateStep (index 2) → jump directly to TypeStep (3).
            goToStep(3)
          }
        }}
        onCancel={goBack}
      />
    </WizardStepLayout>
  )
}
