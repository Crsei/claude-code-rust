import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * "Detected a custom API key in your environment" confirmation dialog.
 *
 * OpenTUI-native port of the upstream `ApproveApiKey`
 * (`ui/examples/upstream-patterns/src/components/ApproveApiKey.tsx`).
 * The upstream version calls `saveGlobalConfig` to remember the
 * approved/rejected truncated key prefix. This Lite-native port
 * surfaces the same confirmation UX but delegates persistence to the
 * caller via `onDone(approved, remember)` — the OpenTUI frontend does
 * not hold the global-config mutation helpers, they live in the Rust
 * backend.
 */

type Props = {
  customApiKeyTruncated: string
  onDone: (approved: boolean) => void
}

const OPTIONS: Array<{ value: 'yes' | 'no'; label: string }> = [
  { value: 'no', label: 'No (recommended)' },
  { value: 'yes', label: 'Yes, use this API key' },
]

export function ApproveApiKey({ customApiKeyTruncated, onDone }: Props) {
  const [selected, setSelected] = useState(0)

  useEffect(() => {
    setSelected(0)
  }, [customApiKeyTruncated])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const input = (seq ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input === 'y') {
      onDone(true)
      return
    }
    if (input === 'n') {
      onDone(false)
      return
    }
    if (name === 'escape') {
      onDone(false)
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || input === 'j' || name === 'tab') {
      setSelected(prev => Math.min(OPTIONS.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const option = OPTIONS[selected]
      if (option) onDone(option.value === 'yes')
    }
  })

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title="Detected a custom API key in your environment"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        <strong>ANTHROPIC_API_KEY</strong>
        {': sk-ant-\u2026'}
        <span fg={c.warning}>{customApiKeyTruncated}</span>
      </text>
      <box marginTop={1}>
        <text>Do you want to use this API key?</text>
      </box>
      <box marginTop={1} flexDirection="column">
        {OPTIONS.map((opt, i) => {
          const isSelected = i === selected
          return (
            <box key={opt.value} flexDirection="row">
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` ${opt.label} `}</strong>
              </text>
              <text fg={c.dim}>{` (${opt.value[0]})`}</text>
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Up/Down to move \u00B7 Enter to confirm \u00B7 y/n hotkeys \u00B7
              Esc = No
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}
