import React, { useCallback, useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../../ipc/context.js'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { useAppDispatch, useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'
import { AgentDetailView } from './AgentDetailView.js'
import { AgentEditor } from './AgentEditor.js'
import { AgentsListView } from './AgentsListView.js'
import { CreateAgentWizard } from './CreateAgentWizard.js'
import { isEditableSource, sourceLabel } from './constants.js'

/**
 * Top-level modal for the `/agents` settings editor. Matches the mode
 * machine from upstream `AgentsMenu.tsx`:
 *   list → detail → (edit | delete-confirm)
 *   list → create (CreateAgentWizard)
 *
 * State changes land via the AgentSettings reducer; the dialog owns only
 * transient navigation state.
 */

type Mode =
  | { kind: 'list' }
  | { kind: 'detail'; entry: AgentDefinitionEntry }
  | { kind: 'edit'; entry: AgentDefinitionEntry }
  | { kind: 'create' }
  | { kind: 'delete-confirm'; entry: AgentDefinitionEntry }

export function AgentsDialog() {
  const { agentSettings } = useAppState()
  const dispatch = useAppDispatch()
  const backend = useBackend()

  const [mode, setMode] = useState<Mode>({ kind: 'list' })
  const [cursor, setCursor] = useState(0)
  const [createNewSelected, setCreateNewSelected] = useState(true)

  // Fetch fresh list + tool catalogue on open.
  useEffect(() => {
    if (!agentSettings.open) return
    setMode({ kind: 'list' })
    setCursor(0)
    setCreateNewSelected(true)
    backend.send({
      type: 'agent_settings_command',
      command: { kind: 'query_list' },
    })
    backend.send({
      type: 'agent_settings_command',
      command: { kind: 'query_tools' },
    })
  }, [agentSettings.open, backend])

  const entries = agentSettings.entries

  const close = useCallback(() => {
    dispatch({ type: 'AGENT_SETTINGS_CLOSE' })
  }, [dispatch])

  // Global keyboard — only active for list / detail / delete-confirm.
  // Sub-modes (`create`, `edit`) own their own keyboard focus.
  useKeyboard(event => {
    if (event.eventType === 'release' || !agentSettings.open) return
    if (mode.kind === 'list') {
      handleListKey(event)
    } else if (mode.kind === 'detail') {
      handleDetailKey(event)
    } else if (mode.kind === 'delete-confirm') {
      handleDeleteKey(event)
    }
  })

  function handleListKey(event: KeyEvent) {
    if (event.name === 'escape') {
      close()
      return
    }
    const totalItems = entries.length + 1
    if (totalItems === 0) return
    const currentIdx = createNewSelected ? 0 : cursor + 1

    if (event.name === 'up') {
      applyListIndex((currentIdx - 1 + totalItems) % totalItems)
    } else if (event.name === 'down' || event.name === 'tab') {
      applyListIndex((currentIdx + 1) % totalItems)
    } else if (event.name === 'return' || event.name === 'enter') {
      if (createNewSelected) {
        setMode({ kind: 'create' })
      } else {
        const entry = entries[cursor]
        if (entry) setMode({ kind: 'detail', entry })
      }
    }
  }

  function applyListIndex(next: number) {
    if (next === 0) {
      setCreateNewSelected(true)
    } else {
      setCreateNewSelected(false)
      setCursor(next - 1)
    }
  }

  function handleDetailKey(event: KeyEvent) {
    if (mode.kind !== 'detail') return
    const input = (event.sequence ?? event.name ?? '').toLowerCase()
    if (event.name === 'escape') {
      setMode({ kind: 'list' })
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      setMode({ kind: 'list' })
      return
    }
    if (!isEditableSource(mode.entry.source)) return
    if (input === 'e') {
      setMode({ kind: 'edit', entry: mode.entry })
    } else if (input === 'd') {
      setMode({ kind: 'delete-confirm', entry: mode.entry })
    }
  }

  function handleDeleteKey(event: KeyEvent) {
    if (mode.kind !== 'delete-confirm') return
    const input = (event.sequence ?? event.name ?? '').toLowerCase()
    if (event.name === 'escape' || input === 'n') {
      setMode({ kind: 'detail', entry: mode.entry })
      return
    }
    if (input === 'y') {
      backend.send({
        type: 'agent_settings_command',
        command: {
          kind: 'delete',
          name: mode.entry.name,
          source: mode.entry.source,
        },
      })
      setMode({ kind: 'list' })
    }
  }

  if (!agentSettings.open) return null

  const title = renderTitle(mode)
  const footer = renderFooter(mode)

  return (
    <box
      position="absolute"
      top={1}
      left={2}
      right={2}
      bottom={2}
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
      title={title}
      titleAlignment="left"
    >
      <box flexDirection="column" flexGrow={1}>
        {mode.kind === 'list' && (
          <AgentsListView
            entries={entries}
            selectedIndex={cursor}
            createNewSelected={createNewSelected}
          />
        )}
        {mode.kind === 'detail' && <AgentDetailView entry={mode.entry} />}
        {mode.kind === 'edit' && (
          <AgentEditor
            entry={mode.entry}
            onDone={() => setMode({ kind: 'list' })}
          />
        )}
        {mode.kind === 'create' && (
          <CreateAgentWizard
            onComplete={() => setMode({ kind: 'list' })}
            onCancel={() => setMode({ kind: 'list' })}
          />
        )}
        {mode.kind === 'delete-confirm' && renderDeleteConfirm(mode.entry)}
      </box>

      <box marginTop={1} flexDirection="column">
        {agentSettings.lastError ? (
          <text><span fg={c.error}>{agentSettings.lastError}</span></text>
        ) : null}
        {agentSettings.lastMessage && !agentSettings.lastError ? (
          <text><span fg={c.success}>{agentSettings.lastMessage}</span></text>
        ) : null}
        <text><span fg={c.dim}>{footer}</span></text>
      </box>
    </box>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type KeyEvent = Parameters<Parameters<typeof useKeyboard>[0]>[0]

function renderTitle(mode: Mode): string {
  switch (mode.kind) {
    case 'list':
      return 'Agents · settings'
    case 'detail':
      return `Agents · ${mode.entry.name} (${sourceLabel(mode.entry.source)})`
    case 'edit':
      return `Agents · edit ${mode.entry.name}`
    case 'create':
      return 'Agents · new agent'
    case 'delete-confirm':
      return `Agents · delete ${mode.entry.name}?`
  }
}

function renderFooter(mode: Mode): string {
  switch (mode.kind) {
    case 'list':
      return '↑/↓ navigate · Enter open · Esc close'
    case 'detail':
      return 'e edit · d delete · Enter / Esc back'
    case 'edit':
      return '↑/↓ choose · Enter select · Esc back'
    case 'create':
      return 'Wizard controls shown per step'
    case 'delete-confirm':
      return 'y delete · n / Esc cancel'
  }
}

function renderDeleteConfirm(entry: AgentDefinitionEntry): React.ReactNode {
  return (
    <box flexDirection="column">
      <text>
        Delete <strong>{entry.name}</strong>{' '}
        <span fg={c.dim}>({sourceLabel(entry.source)})</span>?
      </text>
      <box marginTop={1}>
        <text><span fg={c.dim}>This removes the backing markdown file. There is no undo.</span></text>
      </box>
      <box marginTop={1}>
        <text>
          <span fg={c.error}>[y] yes, delete</span>
          <span fg={c.dim}>{'   '}</span>
          <span fg={c.info}>[n] cancel</span>
        </text>
      </box>
    </box>
  )
}
