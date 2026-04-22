import React from 'react'
import { WizardProvider, type WizardStepComponent } from './wizard/index.js'
import { ColorStep } from './wizard-steps/ColorStep.js'
import { ConfirmStep } from './wizard-steps/ConfirmStep.js'
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
 * Hosts the full 11-step create wizard. Matches upstream
 * `new-agent-creation/CreateAgentWizard.tsx` step ordering exactly so
 * `MethodStep.goToStep(3)` keeps skipping `GenerateStep` for the manual
 * flow. Memory is always included (upstream gates it behind a GrowthBook
 * flag; Full Build exposes it unconditionally).
 */
interface Props {
  onComplete: (message: string) => void
  onCancel: () => void
}

export function CreateAgentWizard({ onComplete, onCancel }: Props) {
  // Each step is a function component that reads/writes wizard state via
  // `useWizard()`. `ConfirmStep` alone needs the `onComplete` closure.
  const ConfirmStepHosted: WizardStepComponent = () => (
    <ConfirmStep onComplete={onComplete} />
  )

  const steps: WizardStepComponent[] = [
    LocationStep, // 0
    MethodStep, // 1
    GenerateStep, // 2 (skipped when method === "manual")
    TypeStep, // 3
    PromptStep, // 4
    DescriptionStep, // 5
    ToolsStep, // 6
    ModelStep, // 7
    ColorStep, // 8
    MemoryStep, // 9
    ConfirmStepHosted, // 10
  ]

  return (
    <WizardProvider
      steps={steps}
      initialData={{ location: 'project', method: 'generate' }}
      onComplete={() => {
        // ConfirmStep owns the final message; this is a no-op.
      }}
      onCancel={onCancel}
      title="Create new agent"
    />
  )
}
