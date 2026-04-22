import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../../ipc/context.js'
import type { AgentDefinitionEntry, AgentDefinitionSource } from '../../ipc/protocol.js'
import { useAppDispatch, useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'
import { AgentDetailView } from './AgentDetailView.js'
import { AgentFormView, type FormField, type FormState } from './AgentFormView.js'
import { AgentsListView } from './AgentsListView.js'
import {
  COLOR_CHOICES,
  MODEL_CHOICES,
  isEditableSource,
  sourceLabel,
} from './constants.js'

/**
 * Top-level modal for the `/agents` settings editor. Mirrors the mode machine
 * from `ui/examples/upstream-patterns/src/components/agents/AgentsMenu.tsx`
 * (list → detail → edit / create / delete-confirm) but in the OpenTUI idiom
 * and against the cc-rust backend protocol.
 *
 * The dialog owns its own transient state (current mode, form buffer, focused
 * field). The agent list itself comes from `state.agentSettings.entries` in
 * the app store — the backend emits that via `AgentSettingsEvent::List` in
 * response to the `query_list` command we fire on mount.
 */

type Mode =
  | { kind: 'list' }
  | { kind: 'detail'; entry: AgentDefinitionEntry }
  | { kind: 'edit'; entry: AgentDefinitionEntry; form: FormState; focused: FormField; error: string | null }
  | { kind: 'create'; form: FormState; focused: FormField; scope: 'user' | 'project'; error: string | null }
  | { kind: 'delete-confirm'; entry: AgentDefinitionEntry }

const FIELD_ORDER: FormField[] = ['name', 'description', 'tools', 'model', 'color', 'prompt']

export function AgentsDialog() {
  const { agentSettings } = useAppState()
  const dispatch = useAppDispatch()
  const backend = useBackend()

  const [mode, setMode] = useState<Mode>({ kind: 'list' })
  const [cursor, setCursor] = useState(0)
  const [createNewSelected, setCreateNewSelected] = useState(true)

  // Fetch the current list every time the dialog opens.
  useEffect(() => {
    if (!agentSettings.open) return
    setMode({ kind: 'list' })
    setCursor(0)
    setCreateNewSelected(true)
    backend.send({
      type: 'agent_settings_command',
      command: { kind: 'query_list' },
    })
  }, [agentSettings.open, backend])

  const entries = agentSettings.entries

  const close = useCallback(() => {
    dispatch({ type: 'AGENT_SETTINGS_CLOSE' })
  }, [dispatch])

  // When the backend confirms a change (upsert / delete), drop back to the
  // list view so the user sees the fresh entry. We key off `lastUpdated` —
  // it only moves forward when `AGENT_SETTINGS_LIST` or `…_CHANGED` fires.
  useEffect(() => {
    if (mode.kind === 'edit' || mode.kind === 'create') {
      const lastMessage = agentSettings.lastMessage
      if (lastMessage) {
        setMode({ kind: 'list' })
        setCreateNewSelected(true)
        setCursor(0)
      }
    }
  }, [agentSettings.lastMessage, agentSettings.lastUpdated])

  // ── Keyboard dispatch ──────────────────────────────────────────────
  useKeyboard(event => {
    if (event.eventType === 'release' || !agentSettings.open) return
    if (event.name === 'escape') {
      onEscape()
      return
    }

    if (mode.kind === 'list') {
      handleListKey(event)
    } else if (mode.kind === 'detail') {
      handleDetailKey(event)
    } else if (mode.kind === 'edit' || mode.kind === 'create') {
      handleFormKey(event)
    } else if (mode.kind === 'delete-confirm') {
      handleDeleteKey(event)
    }
  })

  function onEscape() {
    if (mode.kind === 'list') {
      close()
    } else if (mode.kind === 'detail' || mode.kind === 'delete-confirm') {
      setMode({ kind: 'list' })
    } else {
      // In edit / create, bounce to list without saving.
      setMode({ kind: 'list' })
    }
  }

  function handleListKey(event: KeyEvent) {
    const totalItems = entries.length + 1 // +1 for the "Create new" row
    if (totalItems === 0) return

    const currentIdx = createNewSelected ? 0 : cursor + 1

    if (event.name === 'up') {
      const next = (currentIdx - 1 + totalItems) % totalItems
      applyListIndex(next)
    } else if (event.name === 'down' || event.name === 'tab') {
      const next = (currentIdx + 1) % totalItems
      applyListIndex(next)
    } else if (event.name === 'return' || event.name === 'enter') {
      if (createNewSelected) {
        setMode(buildInitialCreateMode())
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
    if (event.name === 'return' || event.name === 'enter') {
      setMode({ kind: 'list' })
      return
    }
    if (input === 'e' && isEditableSource(mode.entry.source)) {
      setMode(buildEditMode(mode.entry))
    } else if (input === 'd' && isEditableSource(mode.entry.source)) {
      setMode({ kind: 'delete-confirm', entry: mode.entry })
    }
  }

  function handleDeleteKey(event: KeyEvent) {
    if (mode.kind !== 'delete-confirm') return
    const input = (event.sequence ?? event.name ?? '').toLowerCase()
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
    } else if (input === 'n') {
      setMode({ kind: 'detail', entry: mode.entry })
    }
  }

  function handleFormKey(event: KeyEvent) {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    const current = mode
    const focused = current.focused
    const state = current.form

    // Navigation between fields
    if (event.name === 'tab') {
      moveFocus(1)
      return
    }
    if (event.name === 'up' && focused !== 'prompt') {
      moveFocus(-1)
      return
    }
    if (event.name === 'down' && focused !== 'prompt') {
      moveFocus(1)
      return
    }

    // Choice fields (model, color)
    if (focused === 'model' || focused === 'color') {
      const choices = focused === 'model' ? MODEL_CHOICES : COLOR_CHOICES
      const current = focused === 'model' ? state.model : state.color
      const idx = Math.max(0, choices.findIndex(opt => opt.value === current))
      if (event.name === 'left' || event.name === 'right') {
        const next = event.name === 'left'
          ? (idx - 1 + choices.length) % choices.length
          : (idx + 1) % choices.length
        updateForm(f => focused === 'model'
          ? { ...f, model: choices[next]!.value }
          : { ...f, color: choices[next]!.value })
        return
      }
      if (event.name === 'return' || event.name === 'enter') {
        submitForm()
        return
      }
      return
    }

    // Text fields & prompt
    handleTextFieldKey(event, focused)
  }

  function handleTextFieldKey(event: KeyEvent, focused: FormField) {
    const isPrompt = focused === 'prompt'

    if (event.name === 'return' || event.name === 'enter') {
      if (isPrompt || event.shift) {
        // Insert newline in prompt; shift+enter inserts newline anywhere.
        insertAtCursor(focused, '\n')
      } else {
        submitForm()
      }
      return
    }

    if (event.name === 'left') {
      moveCursor(focused, -1)
      return
    }
    if (event.name === 'right') {
      moveCursor(focused, +1)
      return
    }
    if (event.name === 'home') {
      setCursor0(focused, 0)
      return
    }
    if (event.name === 'end') {
      setCursor0(focused, getValue(focused).length)
      return
    }
    if (event.name === 'backspace') {
      deleteAtCursor(focused, -1)
      return
    }
    if (event.name === 'delete') {
      deleteAtCursor(focused, +1)
      return
    }

    // Printable character
    const seq = event.sequence
    if (typeof seq === 'string' && seq.length === 1 && seq >= ' ') {
      insertAtCursor(focused, seq)
    }
  }

  function moveFocus(delta: number) {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    const idx = FIELD_ORDER.indexOf(mode.focused)
    let next = (idx + delta + FIELD_ORDER.length) % FIELD_ORDER.length
    // Skip the locked name field when editing.
    if (mode.kind === 'edit' && FIELD_ORDER[next] === 'name') {
      next = (next + delta + FIELD_ORDER.length) % FIELD_ORDER.length
    }
    updateMode({ focused: FIELD_ORDER[next]! })
  }

  function getValue(field: FormField): string {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return ''
    return readField(mode.form, field)
  }

  function moveCursor(field: FormField, delta: number) {
    const value = getValue(field)
    if (field === 'prompt') {
      if (mode.kind !== 'edit' && mode.kind !== 'create') return
      const next = clamp(mode.form.promptCursor + delta, 0, value.length)
      updateForm(f => ({ ...f, promptCursor: next }))
    } else if (field === 'name' || field === 'description' || field === 'tools') {
      if (mode.kind !== 'edit' && mode.kind !== 'create') return
      const next = clamp(mode.form.cursors[field] + delta, 0, value.length)
      updateForm(f => ({ ...f, cursors: { ...f.cursors, [field]: next } }))
    }
  }

  function setCursor0(field: FormField, pos: number) {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    if (field === 'prompt') {
      updateForm(f => ({ ...f, promptCursor: clamp(pos, 0, f.prompt.length) }))
    } else if (field === 'name' || field === 'description' || field === 'tools') {
      updateForm(f => ({
        ...f,
        cursors: {
          ...f.cursors,
          [field]: clamp(pos, 0, readField(f, field).length),
        },
      }))
    }
  }

  function insertAtCursor(field: FormField, text: string) {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    if (field === 'prompt') {
      updateForm(f => {
        const cursor = clamp(f.promptCursor, 0, f.prompt.length)
        const next = f.prompt.slice(0, cursor) + text + f.prompt.slice(cursor)
        return { ...f, prompt: next, promptCursor: cursor + text.length }
      })
      return
    }
    if (field === 'name' || field === 'description' || field === 'tools') {
      updateForm(f => {
        const value = readField(f, field)
        const cursor = clamp(f.cursors[field], 0, value.length)
        const next = value.slice(0, cursor) + text + value.slice(cursor)
        return writeField(f, field, next, cursor + text.length)
      })
    }
  }

  function deleteAtCursor(field: FormField, direction: -1 | 1) {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    if (field === 'prompt') {
      updateForm(f => {
        const cursor = clamp(f.promptCursor, 0, f.prompt.length)
        if (direction === -1 && cursor === 0) return f
        if (direction === 1 && cursor === f.prompt.length) return f
        const start = direction === -1 ? cursor - 1 : cursor
        const end = direction === -1 ? cursor : cursor + 1
        return {
          ...f,
          prompt: f.prompt.slice(0, start) + f.prompt.slice(end),
          promptCursor: start,
        }
      })
      return
    }
    if (field === 'name' || field === 'description' || field === 'tools') {
      updateForm(f => {
        const value = readField(f, field)
        const cursor = clamp(f.cursors[field], 0, value.length)
        if (direction === -1 && cursor === 0) return f
        if (direction === 1 && cursor === value.length) return f
        const start = direction === -1 ? cursor - 1 : cursor
        const end = direction === -1 ? cursor : cursor + 1
        return writeField(f, field, value.slice(0, start) + value.slice(end), start)
      })
    }
  }

  function updateForm(mut: (f: FormState) => FormState) {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    setMode({ ...mode, form: mut(mode.form), error: null })
  }

  function updateMode(patch: Partial<Extract<Mode, { kind: 'edit' | 'create' }>>) {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    setMode({ ...mode, ...patch } as Mode)
  }

  function submitForm() {
    if (mode.kind !== 'edit' && mode.kind !== 'create') return
    const form = mode.form
    const nameTrim = form.name.trim()
    if (!nameTrim) {
      setMode({ ...mode, error: 'name is required' })
      return
    }
    if (!/^[A-Za-z0-9_-]+$/.test(nameTrim)) {
      setMode({
        ...mode,
        error: 'name may only contain letters, digits, `-`, and `_`',
      })
      return
    }

    const source: AgentDefinitionSource =
      mode.kind === 'edit' ? mode.entry.source : { kind: mode.scope }

    const entry: AgentDefinitionEntry = {
      name: nameTrim,
      description: form.description.trim(),
      system_prompt: form.prompt,
      tools: parseToolsList(form.tools),
      model: form.model.trim() ? form.model.trim() : undefined,
      color: form.color.trim() ? form.color.trim() : undefined,
      source,
      file_path: mode.kind === 'edit' ? mode.entry.file_path : undefined,
    }

    backend.send({
      type: 'agent_settings_command',
      command: { kind: 'upsert', entry },
    })
  }

  // ── Rendering ──────────────────────────────────────────────────────
  if (!agentSettings.open) return null

  const footer = useMemo(() => renderFooter(mode), [mode])
  const title = renderTitle(mode)

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
          <AgentFormView
            mode="edit"
            readOnlyName
            state={mode.form}
            focused={mode.focused}
            error={mode.error}
          />
        )}
        {mode.kind === 'create' && (
          <AgentFormView
            mode="create"
            readOnlyName={false}
            state={mode.form}
            focused={mode.focused}
            error={mode.error}
          />
        )}
        {mode.kind === 'delete-confirm' && renderDeleteConfirm(mode.entry)}
      </box>

      <box marginTop={1} flexDirection="column">
        {agentSettings.lastError ? (
          <text>
            <span fg={c.error}>{agentSettings.lastError}</span>
          </text>
        ) : null}
        {agentSettings.lastMessage && !agentSettings.lastError ? (
          <text>
            <span fg={c.success}>{agentSettings.lastMessage}</span>
          </text>
        ) : null}
        <text>
          <span fg={c.dim}>{footer}</span>
        </text>
      </box>
    </box>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type KeyEvent = Parameters<Parameters<typeof useKeyboard>[0]>[0]

function buildInitialCreateMode(): Extract<Mode, { kind: 'create' }> {
  const form: FormState = {
    name: '',
    description: '',
    tools: '',
    model: '',
    color: '',
    prompt: '',
    cursors: { name: 0, description: 0, tools: 0 },
    promptCursor: 0,
  }
  return { kind: 'create', form, focused: 'name', scope: 'project', error: null }
}

function buildEditMode(entry: AgentDefinitionEntry): Extract<Mode, { kind: 'edit' }> {
  const form: FormState = {
    name: entry.name,
    description: entry.description,
    tools: entry.tools.join(', '),
    model: entry.model ?? '',
    color: entry.color ?? '',
    prompt: entry.system_prompt,
    cursors: {
      name: entry.name.length,
      description: entry.description.length,
      tools: entry.tools.join(', ').length,
    },
    promptCursor: entry.system_prompt.length,
  }
  return { kind: 'edit', entry, form, focused: 'description', error: null }
}

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
    case 'create':
      return 'Tab/↑↓ field · ←/→ choice · Enter save · Esc cancel'
    case 'delete-confirm':
      return 'y delete · n cancel'
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
        <text>
          <span fg={c.dim}>This removes the backing markdown file. There is no undo.</span>
        </text>
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

function readField(f: FormState, field: FormField): string {
  switch (field) {
    case 'name':
      return f.name
    case 'description':
      return f.description
    case 'tools':
      return f.tools
    case 'model':
      return f.model
    case 'color':
      return f.color
    case 'prompt':
      return f.prompt
  }
}

function writeField(
  f: FormState,
  field: 'name' | 'description' | 'tools',
  value: string,
  cursor: number,
): FormState {
  return {
    ...f,
    [field]: value,
    cursors: { ...f.cursors, [field]: cursor },
  }
}

function parseToolsList(raw: string): string[] {
  return raw
    .split(/[,\s]+/)
    .map(t => t.trim())
    .filter(t => t.length > 0)
}

function clamp(n: number, lo: number, hi: number): number {
  if (n < lo) return lo
  if (n > hi) return hi
  return n
}
