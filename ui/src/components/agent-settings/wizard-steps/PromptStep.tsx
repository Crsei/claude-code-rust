import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../../theme.js'
import { WizardStepLayout, useWizard } from '../wizard/index.js'

/**
 * Step 4 — multi-line system-prompt editor. When coming from the Generate
 * path the field is pre-populated with the AI-drafted prompt so the user
 * can tweak it before continuing.
 *
 * Keyboard: Tab → next step, Shift+Tab → previous step; every other
 * printable char (including Enter) inserts into the buffer. Esc cancels.
 */
export function PromptStep() {
  const { wizardData, updateWizardData, goNext, goBack } = useWizard()
  const [value, setValue] = useState(wizardData.systemPrompt ?? '')
  const [cursor, setCursor] = useState(value.length)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'tab' && event.shift) {
      goBack()
      return
    }
    if (event.name === 'tab') {
      updateWizardData({ systemPrompt: value })
      goNext()
      return
    }
    if (event.name === 'escape') {
      goBack()
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
      const pos = cursor
      if (pos === 0) return
      const next = value.slice(0, pos - 1) + value.slice(pos)
      setValue(next)
      setCursor(pos - 1)
      return
    }
    if (event.name === 'delete') {
      const pos = cursor
      if (pos === value.length) return
      setValue(value.slice(0, pos) + value.slice(pos + 1))
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const pos = cursor
      const next = value.slice(0, pos) + '\n' + value.slice(pos)
      setValue(next)
      setCursor(pos + 1)
      return
    }
    const seq = event.sequence
    if (typeof seq === 'string' && seq.length === 1 && seq >= ' ') {
      const pos = cursor
      const next = value.slice(0, pos) + seq + value.slice(pos)
      setValue(next)
      setCursor(pos + 1)
    }
  })

  const before = value.slice(0, cursor)
  const cursorChar = cursor < value.length ? value[cursor]! : ' '
  const after = cursor < value.length ? value.slice(cursor + 1) : ''

  return (
    <WizardStepLayout
      subtitle="System prompt (Tab to continue · Shift+Tab back · Esc cancel)"
      footer={
        wizardData.wasGenerated
          ? 'AI-drafted prompt loaded — edit freely before Tab to continue.'
          : 'Describe how this agent should behave. Enter inserts a newline.'
      }
    >
      <box flexDirection="column" flexGrow={1}>
        <text>
          <span fg={c.text}>{before}</span>
          <span fg={c.bg} bg={c.text}>{cursorChar === '\n' ? ' ' : cursorChar}</span>
          {cursorChar === '\n' ? <span fg={c.text}>{'\n'}</span> : null}
          <span fg={c.text}>{after}</span>
        </text>
      </box>
    </WizardStepLayout>
  )
}
