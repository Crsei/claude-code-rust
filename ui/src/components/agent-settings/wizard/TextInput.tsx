import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../../theme.js'

/**
 * Simple single-line text input used by wizard steps. Keyboard behaviour:
 * - Printable chars insert at the cursor
 * - Left/Right move the cursor; Home/End jump to the ends
 * - Backspace/Delete edit the buffer
 * - Enter calls `onSubmit(value)` — Esc calls `onCancel()` (if provided)
 *
 * The parent owns the value via controlled `value`/`onChange`, mirroring
 * upstream's `TextInput.tsx` contract so the wizard steps can port over
 * with minimal reshaping.
 */

export interface TextInputProps {
  value: string
  onChange: (next: string) => void
  onSubmit: (value: string) => void
  onCancel?: () => void
  placeholder?: string
  active?: boolean
}

export function TextInput({
  value,
  onChange,
  onSubmit,
  onCancel,
  placeholder = '',
  active = true,
}: TextInputProps) {
  const [cursor, setCursor] = useState(value.length)

  useKeyboard(event => {
    if (!active || event.eventType === 'release') return
    if (event.name === 'return' || event.name === 'enter') {
      onSubmit(value)
      return
    }
    if (event.name === 'escape') {
      onCancel?.()
      return
    }
    if (event.name === 'left') {
      setCursor(c => Math.max(0, c - 1))
      return
    }
    if (event.name === 'right') {
      setCursor(c => Math.min(value.length, c + 1))
      return
    }
    if (event.name === 'home') {
      setCursor(0)
      return
    }
    if (event.name === 'end') {
      setCursor(value.length)
      return
    }
    if (event.name === 'backspace') {
      const pos = Math.max(0, Math.min(cursor, value.length))
      if (pos === 0) return
      const next = value.slice(0, pos - 1) + value.slice(pos)
      onChange(next)
      setCursor(pos - 1)
      return
    }
    if (event.name === 'delete') {
      const pos = Math.max(0, Math.min(cursor, value.length))
      if (pos === value.length) return
      const next = value.slice(0, pos) + value.slice(pos + 1)
      onChange(next)
      return
    }
    const seq = event.sequence
    if (typeof seq === 'string' && seq.length === 1 && seq >= ' ') {
      const pos = Math.max(0, Math.min(cursor, value.length))
      const next = value.slice(0, pos) + seq + value.slice(pos)
      onChange(next)
      setCursor(pos + 1)
    }
  })

  if (!active && value.length === 0) {
    return <text><span fg={c.dim}>{placeholder}</span></text>
  }
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
    return <text><span fg={c.text}>{value}</span></text>
  }
  return (
    <text>
      <span fg={c.text}>{before}</span>
      <span fg={c.bg} bg={c.text}>{cursorChar}</span>
      <span fg={c.text}>{after}</span>
    </text>
  )
}
