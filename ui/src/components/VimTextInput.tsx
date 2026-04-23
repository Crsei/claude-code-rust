import React, { useEffect, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * Ported from `ui/examples/upstream-patterns/src/components/VimTextInput.tsx`.
 *
 * Upstream is a thin wrapper around `useVimInput` + `BaseTextInput` —
 * both of which rely on Ink-specific measurement helpers. The Rust port
 * already owns a full vim state machine in `ui/src/vim/`, so this file
 * uses that state machine directly via `initialMode` and a small NORMAL/
 * INSERT dispatcher.
 *
 * Kept simple on purpose: the main composer lives in
 * `components/InputPrompt.tsx` and uses the full `VimState`. This file
 * covers the cases where smaller dialogs want a vim-flavoured input
 * (value, onChange, onSubmit) without taking a dependency on the main
 * composer.
 */

export type VimMode = 'NORMAL' | 'INSERT'

type Props = {
  value: string
  onChange: (next: string) => void
  onSubmit?: (value: string) => void
  onExit?: () => void
  initialMode?: VimMode
  onModeChange?: (mode: VimMode) => void
  focus?: boolean
  placeholder?: string
  showCursor?: boolean
}

export default function VimTextInput({
  value,
  onChange,
  onSubmit,
  onExit,
  initialMode = 'INSERT',
  onModeChange,
  focus = true,
  placeholder,
  showCursor = true,
}: Props): React.ReactElement {
  const [mode, setMode] = useState<VimMode>(initialMode)
  const valueRef = useRef(value)
  valueRef.current = value

  useEffect(() => {
    onModeChange?.(mode)
  }, [mode, onModeChange])

  useEffect(() => {
    setMode(initialMode)
  }, [initialMode])

  useKeyboard((event: KeyEvent) => {
    if (!focus) return
    if (event.eventType === 'release') return

    const name = event.name
    const seq = event.sequence
    const input =
      seq && seq.length === 1 && !event.ctrl && !event.meta
        ? seq
        : name && name.length === 1 && !event.ctrl && !event.meta
          ? name
          : ''

    if (mode === 'INSERT') {
      if (name === 'escape') {
        setMode('NORMAL')
        return
      }
      if (name === 'return' || name === 'enter') {
        onSubmit?.(valueRef.current)
        return
      }
      if (name === 'backspace') {
        if (valueRef.current.length > 0) {
          onChange(valueRef.current.slice(0, -1))
        }
        return
      }
      if (input) {
        onChange(valueRef.current + input)
      }
      return
    }

    if (name === 'escape') {
      onExit?.()
      return
    }
    if (input === 'i' || input === 'a') {
      setMode('INSERT')
      return
    }
    if (input === 'd') {
      onChange('')
      return
    }
    if (name === 'return' || name === 'enter') {
      onSubmit?.(valueRef.current)
    }
  })

  const isEmpty = value.length === 0
  const modeTag = mode === 'NORMAL' ? 'NORMAL' : 'INSERT'
  const modeColor = mode === 'NORMAL' ? c.info : c.accent

  return (
    <box flexDirection="column">
      {isEmpty && placeholder ? (
        <text fg={c.dim}>
          {placeholder}
          {showCursor && focus ? (
            <span fg={c.textBright} bg={modeColor}> </span>
          ) : null}
        </text>
      ) : (
        <text>
          {value}
          {showCursor && focus ? (
            <span fg={c.textBright} bg={modeColor}> </span>
          ) : null}
        </text>
      )}
      <text fg={c.dim}>
        <em>[{modeTag}]</em>
      </text>
    </box>
  )
}
