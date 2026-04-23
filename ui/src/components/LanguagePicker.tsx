import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `LanguagePicker`
 * (`ui/examples/upstream-patterns/src/components/LanguagePicker.tsx`).
 *
 * Lets the user enter their preferred response / voice language. Upstream
 * uses Ink's `TextInput`; here we drive a minimal single-line buffer with
 * `useKeyboard` so the component works without any additional primitives.
 * Enter confirms, Esc cancels, Backspace deletes the last character.
 */

type Props = {
  initialLanguage?: string
  onComplete: (language: string | undefined) => void
  onCancel: () => void
}

export function LanguagePicker({ initialLanguage, onComplete, onCancel }: Props) {
  const [value, setValue] = useState(initialLanguage ?? '')

  useKeyboard(event => {
    if (event.eventType === 'release') return

    const name = event.name
    const sequence = event.sequence

    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'return' || name === 'enter') {
      const trimmed = value.trim()
      onComplete(trimmed.length > 0 ? trimmed : undefined)
      return
    }
    if (name === 'backspace') {
      setValue(prev => prev.slice(0, -1))
      return
    }
    if (
      typeof sequence === 'string' &&
      sequence.length === 1 &&
      sequence >= ' ' &&
      sequence !== '\x7f'
    ) {
      setValue(prev => prev + sequence)
    }
  })

  const shown = value.length === 0
    ? <span fg={c.dim}><em>e.g., Japanese, 日本語, Español…</em></span>
    : <span>{value}</span>

  return (
    <box flexDirection="column" paddingX={1} paddingY={1}>
      <text>Enter your preferred response and voice language:</text>
      <box flexDirection="row" marginTop={1}>
        <box flexShrink={0} minWidth={2}>
          <text fg={c.accent}>{'\u276F '}</text>
        </box>
        <box flexGrow={1}>
          <text>{shown}</text>
        </box>
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>Enter to confirm · Esc to cancel · empty for default (English)</text>
      </box>
    </box>
  )
}
