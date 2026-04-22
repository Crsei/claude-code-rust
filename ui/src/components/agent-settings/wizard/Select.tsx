import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../../theme.js'

/**
 * Minimal arrow-key Select used by every wizard step that asks the user to
 * pick from a small fixed list (Location, Method, Model, Color, Memory).
 * Mirrors upstream `Select` from `CustomSelect/select.tsx` at the behaviour
 * level — callers only need `options`, `onChange`, `onCancel`.
 */

export interface SelectOption<V extends string> {
  value: V
  label: string
  description?: string
}

export interface SelectProps<V extends string> {
  options: SelectOption<V>[]
  onChange: (value: V) => void
  onCancel?: () => void
  /** Optional preselected index. */
  initialIndex?: number
}

export function Select<V extends string>({
  options,
  onChange,
  onCancel,
  initialIndex = 0,
}: SelectProps<V>) {
  const [index, setIndex] = useState(
    Math.max(0, Math.min(initialIndex, options.length - 1)),
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'up') {
      setIndex(i => (i - 1 + options.length) % options.length)
      return
    }
    if (event.name === 'down' || event.name === 'tab') {
      setIndex(i => (i + 1) % options.length)
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const opt = options[index]
      if (opt) onChange(opt.value)
      return
    }
    if (event.name === 'escape' && onCancel) {
      onCancel()
    }
  })

  return (
    <box flexDirection="column">
      {options.map((opt, i) => {
        const selected = i === index
        return (
          <box key={opt.value} flexDirection="column">
            <text>
              <span fg={selected ? c.accent : c.dim}>
                {selected ? '▸ ' : '  '}
              </span>
              <span fg={selected ? c.textBright : c.text}>{opt.label}</span>
            </text>
            {opt.description ? (
              <text>
                <span fg={c.dim}>{'    '}</span>
                <span fg={c.dim}>{opt.description}</span>
              </text>
            ) : null}
          </box>
        )
      })}
    </box>
  )
}
