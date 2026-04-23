import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `InvalidConfigDialog`
 * (`ui/examples/upstream-patterns/src/components/InvalidConfigDialog.tsx`).
 *
 * Shown when the main config file contains invalid JSON. Upstream uses
 * Ink's `Dialog` and kicks off its own `render()` at startup before the
 * normal app boot. Lite does not bootstrap interactive dialogs from the
 * frontend's own entry; instead this component is meant to be displayed
 * inside the app when the backend surfaces a parse error (via a future
 * `config_error` IPC event). The component exposes `onExit` and
 * `onReset` callbacks so the parent can wire up the actual file-write +
 * shutdown behaviour.
 */

type Props = {
  filePath: string
  errorDescription: string
  onExit: () => void
  onReset: () => void
}

type Option = { value: 'exit' | 'reset'; label: string }

const OPTIONS: Option[] = [
  { value: 'exit', label: 'Exit and fix manually' },
  { value: 'reset', label: 'Reset with default configuration' },
]

export function InvalidConfigDialog({
  filePath,
  errorDescription,
  onExit,
  onReset,
}: Props) {
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
        else onReset()
      }
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.error}
      paddingX={2}
      paddingY={1}
      title="Configuration Error"
      titleAlignment="center"
    >
      <box flexDirection="column">
        <text>
          The configuration file at <strong>{filePath}</strong> contains invalid JSON.
        </text>
        <box marginTop={1}>
          <text fg={c.error}>{errorDescription}</text>
        </box>
      </box>
      <box flexDirection="column" marginTop={1}>
        <text>
          <strong>Choose an option:</strong>
        </text>
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
    </box>
  )
}
