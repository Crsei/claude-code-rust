import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../../../ipc/context.js'
import type {
  AgentDefinitionEntry,
  AgentDefinitionSource,
} from '../../../ipc/protocol.js'
import { useAppState } from '../../../store/app-store.js'
import { c } from '../../../theme.js'
import { WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 10 — summary + save. Mirrors upstream `ConfirmStepWrapper.tsx`:
 * shows every field, then offers `Save` / `Save & open in editor`. On
 * success the wizard closes (via `onComplete`) and the dialog falls back
 * to the list view.
 */
interface Props {
  onComplete: (message: string) => void
}

export function ConfirmStep({ onComplete }: Props) {
  const { wizardData, goBack } = useWizard()
  const backend = useBackend()
  const { agentSettings } = useAppState()

  const [action, setAction] = useState<'save' | 'save_and_edit' | null>(null)
  const [cursor, setCursor] = useState(0)

  const actions = [
    { value: 'save' as const, label: 'Save' },
    { value: 'save_and_edit' as const, label: 'Save and open in editor' },
    { value: 'back' as const, label: 'Go back' },
  ]

  // When the backend confirms a change for our agent, the wizard is done.
  useEffect(() => {
    if (action === null) return
    if (agentSettings.lastError) {
      // Surface the error and let the user decide what to do.
      return
    }
    if (
      agentSettings.lastMessage &&
      agentSettings.lastMessage.includes(wizardData.agentType ?? '__never__')
    ) {
      // For "save and edit", additionally fire the editor once the file is
      // on disk.
      if (action === 'save_and_edit') {
        const entry = agentSettings.entries.find(
          e => e.name === wizardData.agentType,
        )
        if (entry?.file_path) {
          backend.send({
            type: 'agent_settings_command',
            command: { kind: 'open_in_editor', file_path: entry.file_path },
          })
        }
      }
      onComplete(`Created agent: ${wizardData.agentType}`)
    }
  }, [
    agentSettings.lastError,
    agentSettings.lastMessage,
    agentSettings.entries,
    action,
    wizardData.agentType,
    backend,
    onComplete,
  ])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'escape') {
      goBack()
      return
    }
    if (event.name === 'up') {
      setCursor(i => (i - 1 + actions.length) % actions.length)
      return
    }
    if (event.name === 'down') {
      setCursor(i => (i + 1) % actions.length)
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const choice = actions[cursor]!
      if (choice.value === 'back') {
        goBack()
        return
      }
      const entry = buildEntry(wizardData)
      if (!entry) return
      setAction(choice.value)
      backend.send({
        type: 'agent_settings_command',
        command: { kind: 'upsert', entry },
      })
    }
  })

  return (
    <WizardStepLayout
      subtitle={`Review and confirm ${wizardData.agentType ?? '(unnamed)'}`}
      footer="↑/↓ navigate · Enter confirm · Esc go back"
    >
      <box flexDirection="column">
        <Summary data={wizardData} />
        <box marginTop={1} flexDirection="column">
          {actions.map((a, i) => (
            <text key={a.value}>
              <span fg={i === cursor ? c.accent : c.dim}>
                {i === cursor ? '▸ ' : '  '}
              </span>
              <span fg={i === cursor ? c.textBright : c.text}>{a.label}</span>
            </text>
          ))}
        </box>
        {agentSettings.lastError ? (
          <box marginTop={1}>
            <text><span fg={c.error}>⚠ {agentSettings.lastError}</span></text>
          </box>
        ) : null}
      </box>
    </WizardStepLayout>
  )
}

function Summary({ data }: { data: ReturnType<typeof useWizard>['wizardData'] }) {
  return (
    <box flexDirection="column">
      <Field label="Name" value={data.agentType ?? '(unset)'} />
      <Field label="Location" value={data.location ?? 'project'} />
      <Field label="Description" value={data.whenToUse ?? '(none)'} />
      <Field
        label="Tools"
        value={
          !data.selectedTools || data.selectedTools.length === 0
            ? 'All tools'
            : data.selectedTools.join(', ')
        }
      />
      <Field label="Model" value={data.model || '(inherit)'} />
      <Field label="Color" value={data.color || '(default)'} />
      {data.memory ? <Field label="Memory" value={data.memory} /> : null}
      {data.wasGenerated ? (
        <text><span fg={c.info}>(AI-drafted prompt)</span></text>
      ) : null}
    </box>
  )
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <text>
      <span fg={c.dim}>{label.padEnd(14, ' ')} </span>
      <span fg={c.text}>{value}</span>
    </text>
  )
}

function buildEntry(
  data: ReturnType<typeof useWizard>['wizardData'],
): AgentDefinitionEntry | null {
  if (!data.agentType || !data.whenToUse || !data.systemPrompt) {
    return null
  }
  const source: AgentDefinitionSource =
    data.location === 'user' ? { kind: 'user' } : { kind: 'project' }
  return {
    name: data.agentType,
    description: data.whenToUse,
    system_prompt: data.systemPrompt,
    tools: data.selectedTools ?? [],
    model: data.model ? data.model : undefined,
    color: data.color ? data.color : undefined,
    memory: data.memory,
    source,
  }
}
