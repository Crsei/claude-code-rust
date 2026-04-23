import React, { useEffect, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/SearchBox.tsx`.
 *
 * Upstream re-exports `@anthropic/ink`'s internal `SearchBox`, which is
 * not available in the Lite frontend. This is a minimal Lite-native
 * equivalent — a single-line text input with the familiar `/ query` look
 * used by the transcript search UI. `onChange` fires on every edit,
 * `onSubmit` on <Enter>, `onCancel` on <Esc>.
 */

type Props = {
  placeholder?: string
  initialValue?: string
  prefix?: string
  isActive?: boolean
  onChange?: (value: string) => void
  onSubmit?: (value: string) => void
  onCancel?: () => void
}

export function SearchBox({
  placeholder = 'Search…',
  initialValue = '',
  prefix = '/',
  isActive = true,
  onChange,
  onSubmit,
  onCancel,
}: Props) {
  const [value, setValue] = useState(initialValue)
  const valueRef = useRef(value)
  valueRef.current = value

  useEffect(() => {
    onChange?.(value)
  }, [value, onChange])

  useKeyboard((event: KeyEvent) => {
    if (!isActive || event.eventType === 'release') return
    const name = event.name

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'return' || name === 'enter') {
      onSubmit?.(valueRef.current)
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setValue(v => v.slice(0, -1))
      return
    }
    const seq = event.sequence
    if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
      setValue(v => v + seq)
    }
  })

  const display = value.length > 0 ? value : placeholder
  const displayColor = value.length > 0 ? c.text : c.dim

  return (
    <box
      flexDirection="row"
      paddingX={1}
      borderStyle="single"
      borderColor={c.dim}
    >
      <text fg={c.accent}>{prefix} </text>
      <text fg={displayColor}>{display}</text>
      {isActive && <text fg={c.accent}>{'\u2588'}</text>}
    </box>
  )
}
