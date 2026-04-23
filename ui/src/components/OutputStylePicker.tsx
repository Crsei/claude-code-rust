import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/OutputStylePicker.tsx`.
 *
 * Upstream loads output styles via `getAllOutputStyles(getCwd())` which
 * reaches into `src/constants/outputStyles` — not compiled into the
 * Lite frontend. Consumers pass the available styles directly; the
 * component renders the picker chrome and routes the selection.
 */

export type OutputStyleOption = {
  value: string
  label: string
  description?: string
}

type Props = {
  initialStyle: string
  options?: OutputStyleOption[]
  isLoading?: boolean
  onComplete: (style: string) => void
  onCancel: () => void
  isStandaloneCommand?: boolean
}

const DEFAULT_OPTIONS: OutputStyleOption[] = [
  {
    value: 'default',
    label: 'Default',
    description:
      'Claude completes coding tasks efficiently and provides concise responses',
  },
]

export function OutputStylePicker({
  initialStyle,
  options = DEFAULT_OPTIONS,
  isLoading = false,
  onComplete,
  onCancel,
  isStandaloneCommand = false,
}: Props) {
  const list = options.length > 0 ? options : DEFAULT_OPTIONS
  const initialIndex = Math.max(
    0,
    list.findIndex(o => o.value === initialStyle),
  )
  const [selected, setSelected] = useState(initialIndex)

  useEffect(() => {
    const next = Math.max(0, list.findIndex(o => o.value === initialStyle))
    setSelected(next)
  }, [initialStyle, list])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release' || isLoading) return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'return' || name === 'enter') {
      const opt = list[selected]
      if (opt) onComplete(opt.value)
      return
    }
    if (name === 'up' || key === 'k') {
      setSelected(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setSelected(idx => Math.min(list.length - 1, idx + 1))
    }
  })

  const chrome = (
    <box flexDirection="column" gap={1}>
      <strong>
        <text fg={c.accent}>Preferred output style</text>
      </strong>
      <text fg={c.dim}>
        This changes how Claude Code communicates with you
      </text>
      {isLoading ? (
        <text fg={c.dim}>Loading output styles…</text>
      ) : (
        <box flexDirection="column">
          {list.map((opt, i) => {
            const isSelected = i === selected
            return (
              <box key={opt.value} flexDirection="column">
                <box flexDirection="row">
                  <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                    <strong>{` ${opt.label} `}</strong>
                  </text>
                </box>
                {opt.description && (
                  <box paddingLeft={3}>
                    <text fg={c.dim}>{opt.description}</text>
                  </box>
                )}
              </box>
            )
          })}
        </box>
      )}
      <text fg={c.dim}>Enter to confirm · Esc to cancel</text>
    </box>
  )

  if (!isStandaloneCommand) {
    return chrome
  }

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
    >
      {chrome}
    </box>
  )
}
