import React, { useEffect } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import type { PermissionOption } from '../../view-model/types.js'

/**
 * Shared button-row + keyboard handler for every permission dialog
 * variant. Lite-native replacement for the sample tree's
 * `PermissionPrompt`
 * (`ui/examples/upstream-patterns/src/components/permissions/PermissionPrompt.tsx`),
 * trimmed to the current protocol: no inline feedback capture, no
 * analytics.
 *
 * The dialog parent passes the normalized `PermissionOption[]` from the
 * adapter (`mapPermissionRequestToViewModel`). When the backend supplied
 * hotkeys (`(y)`, `(n)`, `(a)` suffixes) we honour them; otherwise we
 * fall back to `y` / `n` / `a` based on the label text.
 */

export interface PermissionPromptOptionsProps {
  options: PermissionOption[]
  selectedIndex: number
  onSelect: (index: number) => void
  onConfirm: (option: PermissionOption) => void
  onCancel: () => void
}

function inferFallbackHotkey(label: string): string | undefined {
  const lower = label.toLowerCase()
  if (lower.startsWith('allow') || lower === 'yes') return 'y'
  if (lower.startsWith('deny') || lower === 'no' || lower.startsWith('reject')) {
    return 'n'
  }
  if (lower.startsWith('always')) return 'a'
  return undefined
}

export function resolveHotkey(option: PermissionOption): string | undefined {
  return option.hotkey ?? inferFallbackHotkey(option.label)
}

export function PermissionPromptOptions({
  options,
  selectedIndex,
  onSelect,
  onConfirm,
  onCancel,
}: PermissionPromptOptionsProps) {
  const safeIndex = Math.max(0, Math.min(selectedIndex, options.length - 1))

  // If the incoming selection is out of range (e.g. options shrank),
  // rebase it. Keeps keyboard navigation predictable.
  useEffect(() => {
    if (safeIndex !== selectedIndex) {
      onSelect(safeIndex)
    }
  }, [safeIndex, selectedIndex, onSelect])

  useKeyboard(event => {
    if (event.eventType === 'release') return

    const sequence = event.sequence?.length === 1 ? event.sequence : undefined
    const name = event.name
    const singleName = name?.length === 1 ? name : undefined
    const input = (sequence ?? singleName ?? '').toLowerCase()

    if (input) {
      const matchIndex = options.findIndex(
        opt => resolveHotkey(opt) === input,
      )
      if (matchIndex >= 0) {
        onConfirm(options[matchIndex]!)
        return
      }
    }

    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'left' || input === 'h') {
      onSelect(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'right' || input === 'l') {
      onSelect(Math.min(options.length - 1, safeIndex + 1))
      return
    }
    if (name === 'tab') {
      if (options.length === 0) return
      onSelect((safeIndex + 1) % options.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      const opt = options[safeIndex]
      if (opt) onConfirm(opt)
    }
  })

  if (options.length === 0) {
    return (
      <text fg={c.dim}>
        <em>(no options provided — press Esc to dismiss)</em>
      </text>
    )
  }

  return (
    <box flexDirection="column">
      <box flexDirection="row" gap={2}>
        {options.map((opt, i) => {
          const isSelected = i === safeIndex
          const hotkey = resolveHotkey(opt)
          return (
            <box key={opt.value} flexDirection="row">
              <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                <strong>{` ${opt.label} `}</strong>
              </text>
              {hotkey && <text fg={c.dim}> ({hotkey})</text>}
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Arrow keys or hotkeys to decide. Enter confirms. Esc dismisses.
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}
