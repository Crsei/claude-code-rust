import React, { useMemo } from 'react'
import { c } from '../../theme.js'
import { Select } from '../customselect/select.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/ModelSelector.tsx`.
 *
 * Upstream loads the catalog via `getAgentModelOptions()` from the
 * model settings store. Lite keeps a frontend-safe fallback list and
 * accepts an optional `options` prop so call sites can swap it when a
 * richer catalog is available.
 */

const DEFAULT_MODEL_OPTIONS = [
  { value: 'sonnet', label: 'sonnet', description: 'Balanced reasoning speed' },
  { value: 'opus', label: 'opus', description: 'Deeper reasoning, higher latency' },
  { value: 'haiku', label: 'haiku', description: 'Fast, lightweight responses' },
  { value: 'inherit', label: '(inherit)', description: 'Use the session default' },
] as const

type ModelOption = {
  value: string
  label: string
  description?: string
}

type Props = {
  initialModel?: string
  options?: ModelOption[]
  onComplete: (model?: string) => void
  onCancel?: () => void
}

export function ModelSelector({
  initialModel,
  options,
  onComplete,
  onCancel,
}: Props) {
  const allOptions = useMemo<ModelOption[]>(() => {
    const base: ModelOption[] = options
      ? options.slice()
      : DEFAULT_MODEL_OPTIONS.map(o => ({ ...o }))
    if (initialModel && !base.some(o => o.value === initialModel)) {
      return [
        {
          value: initialModel,
          label: initialModel,
          description: 'Current model (custom ID)',
        },
        ...base,
      ]
    }
    return base
  }, [initialModel, options])

  return (
    <box flexDirection="column">
      <box marginBottom={1}>
        <text fg={c.dim}>
          Model determines the agent&apos;s reasoning capabilities and speed.
        </text>
      </box>
      <Select
        options={allOptions}
        defaultValue={initialModel ?? 'sonnet'}
        onChange={value => {
          onComplete(value === 'inherit' ? undefined : value)
        }}
        onCancel={() => (onCancel ? onCancel() : onComplete(undefined))}
      />
    </box>
  )
}
