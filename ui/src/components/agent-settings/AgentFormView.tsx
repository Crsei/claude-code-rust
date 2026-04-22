import React from 'react'
import { c } from '../../theme.js'
import { COLOR_CHOICES, MODEL_CHOICES } from './constants.js'
import { TextField } from './TextField.js'

/**
 * Edit / create form. Fields are single-line except the system prompt, which
 * is a simple multi-line buffer rendered verbatim (the dialog's keyboard
 * handler feeds characters and `Enter` as newlines when this field is
 * focused).
 *
 * The form is deliberately a dumb renderer; every piece of state (values,
 * cursor positions, focus) is owned by `AgentsDialog` so a single keyboard
 * handler can coordinate field movement with the dialog's modal state.
 */

export type FormField =
  | 'name'
  | 'description'
  | 'tools'
  | 'model'
  | 'color'
  | 'prompt'

export interface FormState {
  name: string
  description: string
  tools: string
  model: string
  color: string
  prompt: string
  /** Cursor offsets for the text fields — prompt has its own below. */
  cursors: Record<'name' | 'description' | 'tools', number>
  promptCursor: number
}

export interface AgentFormViewProps {
  mode: 'create' | 'edit'
  readOnlyName: boolean
  state: FormState
  focused: FormField
  /** Validation message to show under the fields, or `null` if all good. */
  error: string | null
}

export function AgentFormView({
  mode,
  readOnlyName,
  state,
  focused,
  error,
}: AgentFormViewProps) {
  return (
    <box flexDirection="column">
      <text>
        <strong><span fg={c.accent}>{mode === 'create' ? 'Create agent' : 'Edit agent'}</span></strong>
      </text>
      <text>
        <span fg={c.dim}>
          Tab / ↑↓ move · Enter submits from any single-line field · Esc cancels
        </span>
      </text>

      <box marginTop={1} flexDirection="column">
        <TextField
          label="name"
          value={state.name}
          cursor={state.cursors.name}
          active={focused === 'name' && !readOnlyName}
          placeholder="my-agent"
          hint={readOnlyName ? '(name is locked after creation)' : undefined}
        />
        <TextField
          label="description"
          value={state.description}
          cursor={state.cursors.description}
          active={focused === 'description'}
          placeholder="When should the orchestrator delegate?"
        />
        <TextField
          label="tools"
          value={state.tools}
          cursor={state.cursors.tools}
          active={focused === 'tools'}
          placeholder="Read, Grep, Bash (empty = all)"
        />
        {renderChoiceRow('model', 'model', state.model, focused, MODEL_CHOICES)}
        {renderChoiceRow('color', 'color', state.color, focused, COLOR_CHOICES)}
      </box>

      <box marginTop={1} flexDirection="column">
        <text>
          <span fg={focused === 'prompt' ? c.textBright : c.dim}>system prompt </span>
          <span fg={c.dim}>(Shift+Enter for newline on all platforms)</span>
        </text>
        <box paddingLeft={2} flexDirection="column">
          {renderPromptBuffer(state.prompt, state.promptCursor, focused === 'prompt')}
        </box>
      </box>

      {error ? (
        <box marginTop={1}>
          <text>
            <span fg={c.error}>⚠ {error}</span>
          </text>
        </box>
      ) : null}
    </box>
  )
}

function renderChoiceRow(
  label: string,
  field: FormField,
  value: string,
  focused: FormField,
  options: ReadonlyArray<{ value: string; label: string }>,
): React.ReactNode {
  const active = focused === field
  return (
    <box flexDirection="row">
      <text>
        <span fg={active ? c.textBright : c.dim}>{label.padEnd(14, ' ')} </span>
      </text>
      {options.map((opt, i) => {
        const isSelected = opt.value === value
        const fg = isSelected ? (active ? c.bg : c.accent) : c.text
        const bg = isSelected && active ? c.accent : undefined
        return (
          <text key={opt.value || '__empty'}>
            <span fg={fg} bg={bg}>
              {` ${opt.label} `}
            </span>
            {i < options.length - 1 ? <span fg={c.dim}>·</span> : null}
          </text>
        )
      })}
      {active ? (
        <text>
          <span fg={c.dim}>{'  ←/→ pick'}</span>
        </text>
      ) : null}
    </box>
  )
}

function renderPromptBuffer(
  prompt: string,
  cursor: number,
  active: boolean,
): React.ReactNode {
  if (prompt.length === 0) {
    return (
      <text>
        <span fg={c.bg} bg={active ? c.text : c.dim}> </span>
        <span fg="#45475A">Describe how this agent should behave…</span>
      </text>
    )
  }
  if (!active) {
    return <text fg={c.text}>{prompt}</text>
  }
  const clamped = Math.max(0, Math.min(cursor, prompt.length))
  const before = prompt.slice(0, clamped)
  const cursorChar = clamped < prompt.length ? prompt[clamped]! : ' '
  const after = clamped < prompt.length ? prompt.slice(clamped + 1) : ''
  // Newlines are passed through by `<text>` in OpenTUI so multi-line content
  // wraps correctly; we only need to keep the cursor glyph separate.
  return (
    <text>
      <span fg={c.text}>{before}</span>
      <span fg={c.bg} bg={c.text}>{cursorChar === '\n' ? ' ' : cursorChar}</span>
      {cursorChar === '\n' ? <span fg={c.text}>{'\n'}</span> : null}
      <span fg={c.text}>{after}</span>
    </text>
  )
}
