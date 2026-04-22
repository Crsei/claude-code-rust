import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../../ipc/context.js'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import { COLOR_CHOICES, MODEL_CHOICES, isEditableSource, sourceLabel } from './constants.js'
import { ToolSelector } from './ToolSelector.js'
import { Select } from './wizard/index.js'

/**
 * Menu-style editor mirroring upstream `AgentEditor.tsx`. Entries:
 *   * Open in external editor ($EDITOR)
 *   * Edit tools  → ToolSelector
 *   * Edit model  → Select from MODEL_CHOICES
 *   * Edit color  → Select from COLOR_CHOICES
 *
 * Each sub-flow sends an `upsert` when the user confirms so disk and the
 * in-memory entries stay in sync. Callers supply `onDone()` which fires
 * after any sub-flow (including cancels) so the dialog can decide what
 * view to show next.
 */

type SubMode = 'menu' | 'tools' | 'model' | 'color'

interface Props {
  entry: AgentDefinitionEntry
  onDone: () => void
}

export function AgentEditor({ entry, onDone }: Props) {
  const backend = useBackend()
  const [mode, setMode] = useState<SubMode>('menu')
  const [cursor, setCursor] = useState(0)
  const [error, setError] = useState<string | null>(null)

  const editable = isEditableSource(entry.source)
  const items = editable
    ? [
        { id: 'editor', label: 'Open in external editor' },
        { id: 'tools', label: 'Edit tools' },
        { id: 'model', label: 'Edit model' },
        { id: 'color', label: 'Edit color' },
        { id: 'back', label: 'Back' },
      ]
    : [
        { id: 'back', label: 'Back (read-only source)' },
      ]

  useKeyboard(event => {
    if (mode !== 'menu' || event.eventType === 'release') return
    if (event.name === 'escape') {
      onDone()
      return
    }
    if (event.name === 'up') {
      setCursor(i => (i - 1 + items.length) % items.length)
      return
    }
    if (event.name === 'down') {
      setCursor(i => (i + 1) % items.length)
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const item = items[cursor]!
      if (item.id === 'back') {
        onDone()
      } else if (item.id === 'editor') {
        if (!entry.file_path) {
          setError('No file path recorded for this agent')
          return
        }
        backend.send({
          type: 'agent_settings_command',
          command: { kind: 'open_in_editor', file_path: entry.file_path },
        })
        onDone()
      } else {
        setMode(item.id as SubMode)
      }
    }
  })

  const upsertWith = (patch: Partial<AgentDefinitionEntry>) => {
    backend.send({
      type: 'agent_settings_command',
      command: {
        kind: 'upsert',
        entry: { ...entry, ...patch },
      },
    })
    setMode('menu')
    onDone()
  }

  if (mode === 'tools') {
    return (
      <ToolSelector
        initialTools={entry.tools.length === 0 ? undefined : entry.tools}
        onComplete={tools => upsertWith({ tools: tools ?? [] })}
        onCancel={() => setMode('menu')}
      />
    )
  }

  if (mode === 'model') {
    const initial = Math.max(
      0,
      MODEL_CHOICES.findIndex(o => o.value === (entry.model ?? '')),
    )
    return (
      <Select
        initialIndex={initial}
        options={MODEL_CHOICES.map(o => ({
          value: o.value || '__inherit__',
          label: o.label,
        }))}
        onChange={value =>
          upsertWith({ model: value === '__inherit__' ? undefined : value })
        }
        onCancel={() => setMode('menu')}
      />
    )
  }

  if (mode === 'color') {
    const initial = Math.max(
      0,
      COLOR_CHOICES.findIndex(o => o.value === (entry.color ?? '')),
    )
    return (
      <Select
        initialIndex={initial}
        options={COLOR_CHOICES.map(o => ({
          value: o.value || '__none__',
          label: o.label,
        }))}
        onChange={value =>
          upsertWith({ color: value === '__none__' ? undefined : value })
        }
        onCancel={() => setMode('menu')}
      />
    )
  }

  return (
    <box flexDirection="column">
      <text>
        <span fg={c.dim}>Source: </span>
        <span fg={c.text}>{sourceLabel(entry.source)}</span>
      </text>
      {entry.file_path ? (
        <text><span fg={c.dim}>{entry.file_path}</span></text>
      ) : null}
      <box marginTop={1} flexDirection="column">
        {items.map((item, i) => (
          <text key={item.id}>
            <span fg={i === cursor ? c.accent : c.dim}>
              {i === cursor ? '▸ ' : '  '}
            </span>
            <span fg={i === cursor ? c.textBright : c.text}>{item.label}</span>
          </text>
        ))}
      </box>
      {error ? (
        <box marginTop={1}>
          <text><span fg={c.error}>⚠ {error}</span></text>
        </box>
      ) : null}
    </box>
  )
}
