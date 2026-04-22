import React from 'react'
import { c } from '../../theme.js'

/**
 * Inline single-line text field. The cursor column is rendered with
 * an inverted glyph so users can see where keystrokes land; editing is
 * handled by the parent dialog's keyboard handler (we don't run our own
 * `useKeyboard` here because the dialog juggles focus between multiple
 * fields and modal actions).
 *
 * Matches the visual idiom already used by `ComposerBuffer` — distinct
 * from `@opentui/react`'s missing built-in input.
 */

export interface TextFieldProps {
  label: string
  value: string
  cursor: number
  active: boolean
  placeholder?: string
  /** Shown in dim text to the right of the label; useful for validation hints. */
  hint?: string
  /** Widen the label column so multiple fields line up. */
  labelWidth?: number
}

export function TextField({
  label,
  value,
  cursor,
  active,
  placeholder = '',
  hint,
  labelWidth = 14,
}: TextFieldProps) {
  const padded = label.padEnd(labelWidth, ' ')
  return (
    <box flexDirection="row">
      <text>
        <span fg={active ? c.textBright : c.dim}>{padded}</span>
        <span fg={c.dim}>{' '}</span>
      </text>
      {renderBuffer(value, cursor, active, placeholder)}
      {hint ? (
        <text>
          <span fg={c.dim}>{'  '}</span>
          <em><span fg={c.dim}>{hint}</span></em>
        </text>
      ) : null}
    </box>
  )
}

function renderBuffer(value: string, cursor: number, active: boolean, placeholder: string) {
  if (value.length === 0) {
    return (
      <text>
        <span fg={c.bg} bg={active ? c.text : c.dim}> </span>
        <span fg="#45475A">{placeholder}</span>
      </text>
    )
  }
  const clamped = Math.max(0, Math.min(cursor, value.length))
  const before = value.slice(0, clamped)
  const cursorChar = clamped < value.length ? value[clamped]! : ' '
  const after = clamped < value.length ? value.slice(clamped + 1) : ''
  if (!active) {
    return <text fg={c.text}>{value}</text>
  }
  return (
    <text>
      <span fg={c.text}>{before}</span>
      <span fg={c.bg} bg={c.text}>{cursorChar}</span>
      <span fg={c.text}>{after}</span>
    </text>
  )
}
