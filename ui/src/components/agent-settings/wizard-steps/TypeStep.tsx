import React, { useState } from 'react'
import { useAppState } from '../../../store/app-store.js'
import { c } from '../../../theme.js'
import { TextInput, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 3 — identifier for the new agent. Mirrors upstream `TypeStep.tsx`,
 * including the rule that the identifier must match `^[A-Za-z0-9_-]+$` and
 * not collide with an existing agent.
 */
export function TypeStep() {
  const { wizardData, updateWizardData, goNext, goBack } = useWizard()
  const { agentSettings } = useAppState()
  const [value, setValue] = useState(wizardData.agentType ?? '')
  const [error, setError] = useState<string | null>(null)

  const submit = (next: string) => {
    const trimmed = next.trim()
    if (!trimmed) {
      setError('Identifier is required')
      return
    }
    if (!/^[A-Za-z0-9_-]+$/.test(trimmed)) {
      setError('Identifier may only contain letters, digits, `-`, and `_`')
      return
    }
    if (trimmed.length > 64) {
      setError('Identifier must be 64 characters or fewer')
      return
    }
    const clash = agentSettings.entries.find(e => e.name === trimmed)
    if (clash) {
      setError(`An agent named "${trimmed}" already exists (${clash.source.kind})`)
      return
    }
    setError(null)
    updateWizardData({ agentType: trimmed })
    goNext()
  }

  return (
    <WizardStepLayout
      subtitle="Agent type (identifier)"
      footer="Enter continue · Esc go back"
    >
      <box flexDirection="column">
        <text>Enter a unique identifier for your agent:</text>
        <box marginTop={1}>
          <TextInput
            value={value}
            onChange={setValue}
            onSubmit={submit}
            onCancel={goBack}
            placeholder="e.g., test-runner, tech-lead"
          />
        </box>
        {error ? (
          <box marginTop={1}>
            <text><span fg={c.error}>⚠ {error}</span></text>
          </box>
        ) : null}
      </box>
    </WizardStepLayout>
  )
}
