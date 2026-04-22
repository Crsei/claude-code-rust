import React, { useState } from 'react'
import { c } from '../../../theme.js'
import { TextInput, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 5 — the `description` (upstream's `whenToUse`). One-liner the
 * orchestrator reads to decide when to delegate to this agent.
 */
export function DescriptionStep() {
  const { wizardData, updateWizardData, goNext, goBack } = useWizard()
  const [value, setValue] = useState(wizardData.whenToUse ?? '')
  const [error, setError] = useState<string | null>(null)

  const submit = (next: string) => {
    const trimmed = next.trim()
    if (!trimmed) {
      setError('Description is required')
      return
    }
    setError(null)
    updateWizardData({ whenToUse: trimmed })
    goNext()
  }

  return (
    <WizardStepLayout
      subtitle="When should the orchestrator delegate to this agent?"
      footer="Enter continue · Esc go back"
    >
      <box flexDirection="column">
        <TextInput
          value={value}
          onChange={setValue}
          onSubmit={submit}
          onCancel={goBack}
          placeholder="Use this agent when..."
        />
        {error ? (
          <box marginTop={1}>
            <text><span fg={c.error}>⚠ {error}</span></text>
          </box>
        ) : null}
      </box>
    </WizardStepLayout>
  )
}
