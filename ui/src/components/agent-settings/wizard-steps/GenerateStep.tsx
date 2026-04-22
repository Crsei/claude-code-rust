import React, { useEffect, useState } from 'react'
import { useBackend } from '../../../ipc/context.js'
import { useAppDispatch, useAppState } from '../../../store/app-store.js'
import { c } from '../../../theme.js'
import { TextInput, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 2 — AI-assisted generation. Sends `AgentSettingsCommand::Generate`
 * to the backend and waits for the corresponding `Generated` / `Error`
 * event (see the handler in `App.tsx`). On success, the wizard skips
 * directly to the ToolsStep (index 6) to mirror upstream.
 */
export function GenerateStep() {
  const { updateWizardData, goBack, goToStep, wizardData } = useWizard()
  const backend = useBackend()
  const { agentSettings } = useAppState()
  const dispatch = useAppDispatch()

  const [prompt, setPrompt] = useState(wizardData.generationPrompt ?? '')
  const [error, setError] = useState<string | null>(null)

  // When the backend returns a generated agent, thread it into wizard
  // state and jump to ToolsStep. Matches upstream `goToStep(6)`.
  useEffect(() => {
    const gen = agentSettings.lastGenerated
    if (!gen || !wizardData.isGenerating) return
    updateWizardData({
      agentType: gen.identifier,
      whenToUse: gen.whenToUse,
      systemPrompt: gen.systemPrompt,
      isGenerating: false,
      wasGenerated: true,
    })
    dispatch({ type: 'AGENT_SETTINGS_CLEAR_GENERATED' })
    goToStep(6)
  }, [
    agentSettings.lastGenerated,
    wizardData.isGenerating,
    updateWizardData,
    dispatch,
    goToStep,
  ])

  // Surface backend errors inline.
  useEffect(() => {
    if (!wizardData.isGenerating) return
    if (agentSettings.lastError) {
      setError(agentSettings.lastError)
      updateWizardData({ isGenerating: false })
      dispatch({ type: 'AGENT_SETTINGS_CLEAR_NOTICE' })
    }
  }, [
    agentSettings.lastError,
    wizardData.isGenerating,
    updateWizardData,
    dispatch,
  ])

  const submit = (value: string) => {
    const trimmed = value.trim()
    if (!trimmed) {
      setError('Please describe what the agent should do')
      return
    }
    setError(null)
    updateWizardData({ generationPrompt: trimmed, isGenerating: true })
    const existingNames = agentSettings.entries.map(e => e.name)
    backend.send({
      type: 'agent_settings_command',
      command: { kind: 'generate', user_prompt: trimmed, existing_names: existingNames },
    })
  }

  const isGenerating =
    wizardData.isGenerating === true || agentSettings.generating

  if (isGenerating) {
    return (
      <WizardStepLayout
        subtitle="Generating agent from description…"
        footer="This usually takes 5–15 seconds. Esc to cancel."
      >
        <text>
          <span fg={c.accent}>◐</span>
          <span fg={c.dim}>{' waiting for the model to respond'}</span>
        </text>
      </WizardStepLayout>
    )
  }

  return (
    <WizardStepLayout
      subtitle="Describe what this agent should do (be specific — the model reads every word)"
      footer="Enter submit · Esc go back"
    >
      <box flexDirection="column">
        {error ? (
          <box marginBottom={1}>
            <text><span fg={c.error}>⚠ {error}</span></text>
          </box>
        ) : null}
        <TextInput
          value={prompt}
          onChange={setPrompt}
          onSubmit={submit}
          onCancel={() => {
            updateWizardData({
              generationPrompt: '',
              agentType: '',
              systemPrompt: '',
              whenToUse: '',
              wasGenerated: false,
            })
            goBack()
          }}
          placeholder="e.g., Help me write unit tests for my code…"
        />
      </box>
    </WizardStepLayout>
  )
}
