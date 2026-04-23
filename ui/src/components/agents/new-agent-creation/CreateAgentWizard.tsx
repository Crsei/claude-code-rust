import React, { useCallback, useState } from 'react'
import { c } from '../../../theme.js'
import type { AgentDefinitionEntry } from '../../../ipc/protocol.js'
import type { ToolSpec } from '../ToolSelector.js'
import type { DraftAgent } from '../types.js'
import { ColorStep } from './wizard-steps/ColorStep.js'
import { ConfirmStep } from './wizard-steps/ConfirmStep.js'
import { ConfirmStepWrapper } from './wizard-steps/ConfirmStepWrapper.js'
import { DescriptionStep } from './wizard-steps/DescriptionStep.js'
import { GenerateStep } from './wizard-steps/GenerateStep.js'
import { LocationStep } from './wizard-steps/LocationStep.js'
import { MemoryStep } from './wizard-steps/MemoryStep.js'
import { MethodStep } from './wizard-steps/MethodStep.js'
import { ModelStep } from './wizard-steps/ModelStep.js'
import { PromptStep } from './wizard-steps/PromptStep.js'
import { ToolsStep } from './wizard-steps/ToolsStep.js'
import { TypeStep } from './wizard-steps/TypeStep.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/new-agent-creation/CreateAgentWizard.tsx`.
 *
 * Sequences the twelve wizard steps. Upstream composes the same order —
 * `location → method → (generate | type → description → prompt) →
 * tools → model → color → memory → confirm`. The Lite port keeps the
 * step IDs and signature so each step component is swap-compatible
 * with upstream.
 */

type StepId =
  | 'location'
  | 'method'
  | 'generate'
  | 'type'
  | 'description'
  | 'prompt'
  | 'tools'
  | 'model'
  | 'color'
  | 'memory'
  | 'confirm'

type Props = {
  availableTools: ToolSpec[]
  existingAgents: AgentDefinitionEntry[]
  onDone: (draft: DraftAgent) => void
  onCancel: () => void
}

const EMPTY_DRAFT: DraftAgent = {
  agentType: '',
  description: '',
  systemPrompt: '',
  tools: undefined,
  model: undefined,
  color: undefined,
  memory: undefined,
  source: 'projectSettings',
}

export function CreateAgentWizard({
  availableTools,
  existingAgents,
  onDone,
  onCancel,
}: Props) {
  const [draft, setDraft] = useState<DraftAgent>(EMPTY_DRAFT)
  const [step, setStep] = useState<StepId>('location')
  const [method, setMethod] = useState<'generate' | 'manual'>('manual')

  const update = useCallback((patch: Partial<DraftAgent>) => {
    setDraft(prev => ({ ...prev, ...patch }))
  }, [])

  const go = (next: StepId) => setStep(next)

  const footer = (
    <box marginTop={1}>
      <text fg={c.dim}>Esc to cancel · Enter to advance</text>
    </box>
  )

  if (step === 'location') {
    return (
      <box flexDirection="column">
        <LocationStep
          value={draft.source}
          onSelect={source => {
            update({ source })
            go('method')
          }}
          onCancel={onCancel}
        />
        {footer}
      </box>
    )
  }

  if (step === 'method') {
    return (
      <box flexDirection="column">
        <MethodStep
          value={method}
          onSelect={choice => {
            setMethod(choice)
            go(choice === 'generate' ? 'generate' : 'type')
          }}
          onCancel={onCancel}
        />
        {footer}
      </box>
    )
  }

  if (step === 'generate') {
    return (
      <GenerateStep
        description={draft.description}
        onApply={patch => {
          update(patch)
          go('tools')
        }}
        onCancel={onCancel}
      />
    )
  }

  if (step === 'type') {
    return (
      <TypeStep
        existingAgents={existingAgents}
        value={draft.agentType}
        onSubmit={agentType => {
          update({ agentType })
          go('description')
        }}
        onCancel={onCancel}
      />
    )
  }

  if (step === 'description') {
    return (
      <DescriptionStep
        value={draft.description}
        onSubmit={description => {
          update({ description })
          go('prompt')
        }}
        onCancel={onCancel}
      />
    )
  }

  if (step === 'prompt') {
    return (
      <PromptStep
        value={draft.systemPrompt}
        onSubmit={systemPrompt => {
          update({ systemPrompt })
          go('tools')
        }}
        onCancel={onCancel}
      />
    )
  }

  if (step === 'tools') {
    return (
      <ToolsStep
        availableTools={availableTools}
        selected={draft.tools}
        onSubmit={tools => {
          update({ tools })
          go('model')
        }}
        onCancel={onCancel}
      />
    )
  }

  if (step === 'model') {
    return (
      <ModelStep
        value={draft.model}
        onSubmit={model => {
          update({ model })
          go('color')
        }}
        onCancel={onCancel}
      />
    )
  }

  if (step === 'color') {
    return (
      <ColorStep
        agentName={draft.agentType || 'agent'}
        value={draft.color}
        onSubmit={color => {
          update({ color: color ?? undefined })
          go('memory')
        }}
        onCancel={onCancel}
      />
    )
  }

  if (step === 'memory') {
    return (
      <MemoryStep
        value={draft.memory}
        onSubmit={memory => {
          update({ memory })
          go('confirm')
        }}
        onCancel={onCancel}
      />
    )
  }

  return (
    <ConfirmStepWrapper
      availableTools={availableTools.map(t => t.name)}
      existingAgents={existingAgents}
      draft={draft}
      onConfirm={() => onDone(draft)}
      onBack={() => go('memory')}
    >
      <ConfirmStep draft={draft} />
    </ConfirmStepWrapper>
  )
}
