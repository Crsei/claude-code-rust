import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { ColorPicker, type AgentColorName } from './ColorPicker.js'
import { ModelSelector } from './ModelSelector.js'
import { ToolSelector, type ToolSpec } from './ToolSelector.js'
import type { DraftAgent } from './types.js'
import { validateAgent } from './validateAgent.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/AgentEditor.tsx`.
 *
 * In-place edit form for an existing (user- or project-sourced) agent.
 * Upstream renders a richer multi-pane form; the Lite port keeps the
 * same fields — name, description, tools, model, color, system prompt —
 * but drives the sub-pickers via panel swapping instead of inline
 * focus capture. Callers receive the fully-validated draft via
 * `onSave`.
 */

type Panel = 'form' | 'tools' | 'model' | 'color'

type Props = {
  agent: AgentDefinitionEntry
  availableTools: ToolSpec[]
  existingAgents: AgentDefinitionEntry[]
  onSave: (draft: DraftAgent) => void
  onCancel: () => void
}

function entryToDraft(entry: AgentDefinitionEntry): DraftAgent {
  return {
    agentType: entry.name,
    description: entry.description ?? '',
    systemPrompt: entry.system_prompt ?? '',
    tools: entry.tools,
    model: entry.model ?? undefined,
    color: entry.color ?? undefined,
    memory: entry.memory ?? undefined,
    permissionMode: entry.permission_mode ?? undefined,
    source: (entry.source?.kind === 'user'
      ? 'userSettings'
      : entry.source?.kind === 'project'
        ? 'projectSettings'
        : 'userSettings') as DraftAgent['source'],
  }
}

export function AgentEditor({
  agent,
  availableTools,
  existingAgents,
  onSave,
  onCancel,
}: Props) {
  const [draft, setDraft] = useState<DraftAgent>(() => entryToDraft(agent))
  const [panel, setPanel] = useState<Panel>('form')
  const [focus, setFocus] = useState(0)

  const fields: Array<{
    key: keyof DraftAgent | 'tools-button' | 'model-button' | 'color-button'
    label: string
  }> = [
    { key: 'agentType', label: 'Name' },
    { key: 'description', label: 'Description' },
    { key: 'systemPrompt', label: 'System prompt' },
    { key: 'tools-button', label: 'Tools' },
    { key: 'model-button', label: 'Model' },
    { key: 'color-button', label: 'Color' },
  ]

  const validation = validateAgent(
    draft,
    availableTools.map(t => t.name),
    existingAgents,
  )

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release' || panel !== 'form') return
    const name = event.name
    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'up') {
      setFocus(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down') {
      setFocus(idx => Math.min(fields.length - 1, idx + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const field = fields[focus]
      if (!field) return
      if (field.key === 'tools-button') setPanel('tools')
      else if (field.key === 'model-button') setPanel('model')
      else if (field.key === 'color-button') setPanel('color')
      else if (validation.isValid) onSave(draft)
      return
    }
    const seq = event.sequence
    const field = fields[focus]
    if (!field) return
    if (field.key === 'agentType' || field.key === 'description' || field.key === 'systemPrompt') {
      const key = field.key
      if (name === 'backspace' || name === 'delete') {
        setDraft(d => ({ ...d, [key]: String(d[key] ?? '').slice(0, -1) }))
        return
      }
      if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
        setDraft(d => ({ ...d, [key]: String(d[key] ?? '') + seq }))
      }
    }
  })

  if (panel === 'tools') {
    return (
      <ToolSelector
        tools={availableTools}
        selected={draft.tools}
        onComplete={tools => {
          setDraft(d => ({ ...d, tools }))
          setPanel('form')
        }}
        onCancel={() => setPanel('form')}
      />
    )
  }

  if (panel === 'model') {
    return (
      <ModelSelector
        initialModel={draft.model}
        onComplete={model => {
          setDraft(d => ({ ...d, model }))
          setPanel('form')
        }}
        onCancel={() => setPanel('form')}
      />
    )
  }

  if (panel === 'color') {
    return (
      <ColorPicker
        agentName={draft.agentType}
        currentColor={(draft.color as AgentColorName | undefined) ?? 'automatic'}
        onConfirm={color => {
          setDraft(d => ({ ...d, color: color ?? undefined }))
          setPanel('form')
        }}
        onCancel={() => setPanel('form')}
      />
    )
  }

  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Edit agent</text></strong>

      {fields.map((field, i) => {
        const isFocused = i === focus
        if (
          field.key === 'tools-button' ||
          field.key === 'model-button' ||
          field.key === 'color-button'
        ) {
          const preview =
            field.key === 'tools-button'
              ? draft.tools === undefined
                ? 'All tools'
                : draft.tools.length === 0
                  ? 'None'
                  : `${draft.tools.length} selected`
              : field.key === 'model-button'
                ? draft.model ?? '(inherit)'
                : draft.color ?? 'automatic'
          return (
            <box key={field.key} flexDirection="row" gap={1}>
              <text fg={isFocused ? c.accent : c.dim}>
                {isFocused ? '\u276F' : ' '}
              </text>
              <strong><text>{field.label}:</text></strong>
              <text fg={c.text}>{preview}</text>
              <text fg={c.dim}>(Enter to edit)</text>
            </box>
          )
        }
        const value = String(draft[field.key] ?? '')
        return (
          <box key={field.key} flexDirection="row" gap={1}>
            <text fg={isFocused ? c.accent : c.dim}>
              {isFocused ? '\u276F' : ' '}
            </text>
            <strong><text>{field.label}:</text></strong>
            <text fg={c.text}>{value || ' '}</text>
            {isFocused && <text fg={c.accent}>{'\u2588'}</text>}
          </box>
        )
      })}

      {validation.errors.length > 0 && (
        <box flexDirection="column" marginTop={1}>
          {validation.errors.map((err, i) => (
            <text key={i} fg={c.error}>• {err}</text>
          ))}
        </box>
      )}
      {validation.warnings.length > 0 && (
        <box flexDirection="column">
          {validation.warnings.map((warn, i) => (
            <text key={i} fg={c.warning}>• {warn}</text>
          ))}
        </box>
      )}

      <box marginTop={1}>
        <text fg={c.dim}>
          {validation.isValid
            ? 'Enter to save · Esc to cancel'
            : 'Resolve errors to save · Esc to cancel'}
        </text>
      </box>
    </box>
  )
}
