import React, { useEffect, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/select-input-option.tsx`.
 *
 * Upstream's variant rides on top of Ink's `BaseTextInput` with paste
 * handling, cursor tracking, and input-mode coupling with the parent
 * `Select`. The Lite port keeps the feature set that Lite callers
 * actually use:
 *
 *  - render a label prefix + current value + blinking cursor,
 *  - capture keystrokes while the row is focused,
 *  - call `onChange` on Enter, `onCancel` on Esc,
 *  - honour `allowEmptySubmitToCancel`.
 *
 * The component expects the owning `<Select>` to gate input by setting
 * `isActive` (true only while the input row is focused) so sibling rows
 * don't double-process keystrokes.
 */

type Props = {
  label: React.ReactNode
  initialValue?: string
  placeholder?: string
  isActive?: boolean
  /** When true, submitting empty text still fires `onChange` instead of
   *  `onCancel`. Matches the upstream flag. */
  allowEmptySubmitToCancel?: boolean
  /** Always render the label beside the value, even when the label
   *  would normally be hidden while editing. */
  showLabelWithValue?: boolean
  /** Separator string between the label and value. */
  labelValueSeparator?: string
  onChange: (value: string) => void
  onCancel?: () => void
  /** When set, resets the cursor to the end of the input whenever the
   *  value or focus changes. */
  resetCursorOnUpdate?: boolean
}

export function SelectInputOption({
  label,
  initialValue = '',
  placeholder,
  isActive = false,
  allowEmptySubmitToCancel = false,
  showLabelWithValue = false,
  labelValueSeparator = ', ',
  onChange,
  onCancel,
  resetCursorOnUpdate = false,
}: Props) {
  const [value, setValue] = useState(initialValue)
  const valueRef = useRef(value)
  valueRef.current = value

  useEffect(() => {
    if (resetCursorOnUpdate) setValue(initialValue)
  }, [initialValue, resetCursorOnUpdate])

  useKeyboard((event: KeyEvent) => {
    if (!isActive || event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'return' || name === 'enter') {
      const trimmed = valueRef.current
      if (trimmed.trim().length === 0 && !allowEmptySubmitToCancel) {
        onCancel?.()
        return
      }
      onChange(trimmed)
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setValue(v => v.slice(0, -1))
      return
    }
    if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
      setValue(v => v + seq)
    }
  })

  const displayValue = value.length > 0 ? value : placeholder ?? ''
  const displayColor = value.length > 0 ? c.text : c.dim

  return (
    <box flexDirection="row" gap={1}>
      {(showLabelWithValue || value.length === 0) && (
        <text>{label}</text>
      )}
      {showLabelWithValue && <text fg={c.dim}>{labelValueSeparator}</text>}
      <text fg={displayColor}>{displayValue}</text>
      {isActive && <text fg={c.accent}>{'\u2588'}</text>}
    </box>
  )
}
