import React, { useRef } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * Ported from `ui/examples/upstream-patterns/src/components/TextInput.tsx`.
 *
 * Upstream pulls in `useTextInput`, the voice-mode EMA waveform cursor,
 * clipboard-image hinting, and Ink's `BaseTextInput`. None of that
 * machinery exists in the Rust port — the composer lives in
 * `components/PromptInput/*` — so this is a focused text input used by
 * smaller dialogs (theme picker preview, custom statusline command,
 * etc.). It keeps the same outward shape (`value`, `onChange`,
 * `onSubmit`, `placeholder`, `showCursor`, `mask`) so call sites can
 * mix-and-match with the upstream flow without rewriting their props.
 */

export type TextHighlight = {
  start: number
  length: number
  color?: string
}

type Props = {
  value: string
  onChange: (next: string) => void
  onSubmit?: (value: string) => void
  /** Called when the user hits Escape. */
  onExit?: () => void
  /** Mask every typed character (e.g. for secrets). */
  mask?: string
  placeholder?: string
  /** When false, the input doesn't reserve a cursor column. */
  showCursor?: boolean
  /** When false, `useKeyboard` no-ops — callers can park the input. */
  focus?: boolean
  /** Optional inline highlights (compat with upstream's highlighter API). */
  highlights?: TextHighlight[]
}

function renderMasked(value: string, mask: string | undefined): string {
  if (!mask) return value
  return mask.repeat(value.length)
}

function applyHighlights(
  text: string,
  highlights?: TextHighlight[],
): React.ReactNode[] {
  if (!highlights || highlights.length === 0) {
    return [text]
  }
  const sorted = [...highlights].sort((a, b) => a.start - b.start)
  const parts: React.ReactNode[] = []
  let cursor = 0
  sorted.forEach((h, i) => {
    if (h.start > cursor) {
      parts.push(text.slice(cursor, h.start))
    }
    const segment = text.slice(h.start, h.start + h.length)
    parts.push(
      <span key={`h-${i}`} fg={h.color ?? c.warning}>
        {segment}
      </span>,
    )
    cursor = h.start + h.length
  })
  if (cursor < text.length) {
    parts.push(text.slice(cursor))
  }
  return parts
}

export default function TextInput({
  value,
  onChange,
  onSubmit,
  onExit,
  mask,
  placeholder,
  showCursor = true,
  focus = true,
  highlights,
}: Props): React.ReactElement {
  const valueRef = useRef(value)
  valueRef.current = value

  useKeyboard((event: KeyEvent) => {
    if (!focus) return
    if (event.eventType === 'release') return
    const name = event.name

    if (name === 'escape') {
      onExit?.()
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

    const seq = event.sequence
    const input =
      seq && seq.length === 1 && !event.ctrl && !event.meta
        ? seq
        : name && name.length === 1 && !event.ctrl && !event.meta
          ? name
          : ''
    if (input) {
      onChange(valueRef.current + input)
    }
  })

  const displayValue = renderMasked(value, mask)
  const isEmpty = displayValue.length === 0

  return (
    <box flexDirection="row">
      {isEmpty && placeholder ? (
        <text fg={c.dim}>
          {placeholder}
          {showCursor && focus ? (
            <span fg={c.textBright} bg={c.accent}> </span>
          ) : null}
        </text>
      ) : (
        <text>
          {applyHighlights(displayValue, highlights)}
          {showCursor && focus ? (
            <span fg={c.textBright} bg={c.accent}> </span>
          ) : null}
        </text>
      )}
    </box>
  )
}
