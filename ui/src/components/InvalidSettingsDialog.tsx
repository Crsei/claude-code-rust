import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import {
  ValidationErrorsList,
  type ValidationError,
} from './ValidationErrorsList.js'

/**
 * OpenTUI port of upstream `InvalidSettingsDialog`
 * (`ui/examples/upstream-patterns/src/components/InvalidSettingsDialog.tsx`).
 *
 * Shown at startup when settings files fail validation. User picks
 * between "continue without these settings" and "exit and fix". Upstream
 * reaches for Ink's `Dialog` + `Select`; Lite composes the same shape
 * from OpenTUI primitives and an inline arrow-key picker (same pattern
 * as `agent-settings/wizard/Select.tsx`).
 */

type Props = {
  settingsErrors: ValidationError[]
  onContinue: () => void
  onExit: () => void
}

type Option = { value: 'continue' | 'exit'; label: string }

const OPTIONS: Option[] = [
  { value: 'exit', label: 'Exit and fix manually' },
  { value: 'continue', label: 'Continue without these settings' },
]

export function InvalidSettingsDialog({ settingsErrors, onContinue, onExit }: Props) {
  const [index, setIndex] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    switch (event.name) {
      case 'up':
        setIndex(i => (i - 1 + OPTIONS.length) % OPTIONS.length)
        return
      case 'down':
      case 'tab':
        setIndex(i => (i + 1) % OPTIONS.length)
        return
      case 'escape':
        onExit()
        return
      case 'return':
      case 'enter': {
        const opt = OPTIONS[index]
        if (!opt) return
        if (opt.value === 'exit') onExit()
        else onContinue()
      }
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
      title="Settings Error"
      titleAlignment="center"
    >
      <ValidationErrorsList errors={settingsErrors} />
      <box marginTop={1}>
        <text fg={c.dim}>
          Files with errors are skipped entirely, not just the invalid settings.
        </text>
      </box>
      <box flexDirection="column" marginTop={1}>
        {OPTIONS.map((opt, i) => {
          const selected = i === index
          return (
            <text key={opt.value}>
              <span fg={selected ? c.accent : c.dim}>
                {selected ? '\u25B8 ' : '  '}
              </span>
              <span fg={selected ? c.textBright : c.text}>{opt.label}</span>
            </text>
          )
        })}
      </box>
    </box>
  )
}
