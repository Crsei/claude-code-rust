import React from 'react'
import type { AgentMemoryScope } from '../../../ipc/protocol.js'
import { Select, WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 9 — memory scope. Mirrors upstream `MemoryStep.tsx` (guarded behind
 * `isAutoMemoryEnabled()` in upstream; we always include it in Full Build
 * since agent memory is already shipped on the backend).
 */
export function MemoryStep() {
  const { wizardData, updateWizardData, goNext, goBack } = useWizard()
  const options: Array<{ value: AgentMemoryScope | '__none__'; label: string; description: string }> = [
    {
      value: '__none__',
      label: 'Disabled',
      description: 'Agent keeps no auto-memory across runs.',
    },
    {
      value: 'project',
      label: 'Project',
      description: 'Writes land under the current project’s agent memory.',
    },
    {
      value: 'user',
      label: 'User',
      description: 'Writes land in the user-global agent memory.',
    },
    {
      value: 'local',
      label: 'Local (session-only)',
      description: 'Scratch memory for the current session — not persisted.',
    },
  ]
  const initial = Math.max(
    0,
    options.findIndex(
      o => (wizardData.memory ?? '__none__') === o.value,
    ),
  )
  return (
    <WizardStepLayout
      subtitle="Agent memory scope"
      footer="↑/↓ navigate · Enter select · Esc go back"
    >
      <Select
        initialIndex={initial}
        options={options}
        onChange={value => {
          updateWizardData({
            memory: value === '__none__' ? undefined : (value as AgentMemoryScope),
          })
          goNext()
        }}
        onCancel={goBack}
      />
    </WizardStepLayout>
  )
}
