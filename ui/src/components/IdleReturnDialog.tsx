import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/IdleReturnDialog.tsx`.
 *
 * Upstream uses `formatTokens` + `Select` to prompt the user whether
 * they want to continue a long-idle conversation, clear context, or
 * opt out permanently. cc-rust lacks both helpers today; we inline a
 * minimal k-formatted token helper and run our own vertical selector
 * mirroring the permission dialog layout.
 *
 * The dialog is purely view. Callers pass the idle duration and the
 * recent token spend; picking an option fires `onDone(action)` with
 * one of the four discriminants so the parent owns persistence
 * (clearing the conversation, writing "never ask again", …).
 */

export type IdleReturnAction = 'continue' | 'clear' | 'dismiss' | 'never'

type Props = {
  idleMinutes: number
  totalInputTokens: number
  onDone: (action: IdleReturnAction) => void
}

type Option = {
  value: Exclude<IdleReturnAction, 'dismiss'>
  label: string
  hotkey: string
}

const OPTIONS: Option[] = [
  { value: 'continue', label: 'Continue this conversation', hotkey: 'c' },
  { value: 'clear', label: 'Send message as a new conversation', hotkey: 'n' },
  { value: 'never', label: "Don't ask me again", hotkey: 'x' },
]

function formatTokens(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return '0'
  if (value < 1000) return String(value)
  if (value < 10_000) return `${(value / 1000).toFixed(1)}k`
  if (value < 1_000_000) return `${Math.round(value / 1000)}k`
  return `${(value / 1_000_000).toFixed(1)}m`
}

function formatIdleDuration(minutes: number): string {
  if (minutes < 1) return '< 1m'
  if (minutes < 60) return `${Math.floor(minutes)}m`
  const hours = Math.floor(minutes / 60)
  const remainingMinutes = Math.floor(minutes % 60)
  if (remainingMinutes === 0) return `${hours}h`
  return `${hours}h ${remainingMinutes}m`
}

export function IdleReturnDialog({ idleMinutes, totalInputTokens, onDone }: Props) {
  const [selected, setSelected] = useState(0)
  const options = OPTIONS
  const safeIndex = Math.max(0, Math.min(selected, options.length - 1))

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input) {
      const match = options.findIndex(opt => opt.hotkey === input)
      if (match >= 0) {
        onDone(options[match]!.value)
        return
      }
    }

    if (name === 'escape') {
      onDone('dismiss')
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setSelected(Math.min(options.length - 1, safeIndex + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      onDone(options[safeIndex]!.value)
    }
  })

  const title = useMemo(
    () =>
      `You've been away ${formatIdleDuration(idleMinutes)} and this conversation is ${formatTokens(totalInputTokens)} tokens.`,
    [idleMinutes, totalInputTokens],
  )

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.info}
      title={title}
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        <span fg={c.dim}>
          If this is a new task, clearing context will save usage and be faster.
        </span>
      </text>
      <box marginTop={1} flexDirection="column">
        {options.map((opt, i) => {
          const isSelected = i === safeIndex
          return (
            <box key={opt.value} flexDirection="row">
              <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                <strong>{` ${opt.label} `}</strong>
              </text>
              <text fg={c.dim}> ({opt.hotkey})</text>
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Up/Down to move · Enter to confirm · Esc to dismiss
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}
