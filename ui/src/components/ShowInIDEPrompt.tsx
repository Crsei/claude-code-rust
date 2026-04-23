import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/ShowInIDEPrompt.tsx`.
 *
 * Rendered when a file-edit permission request is being visualised in
 * an IDE diff view and the user must decide from the terminal. Mirrors
 * the upstream chrome (title with the IDE name, filename, option list,
 * symlink / save hints) using OpenTUI primitives.
 */

export type PermissionOption = {
  value: string
  label: string
  hotkey?: string
  /** Tagged union used by the parent flow to disambiguate accept vs
   *  reject without re-parsing the label. */
  option: { type: 'accept' | 'accept-once' | 'reject' | string }
}

type Props<A> = {
  filePath: string
  input: A
  onChange: (option: PermissionOption['option'], args: A, feedback?: string) => void
  options: PermissionOption[]
  ideName: string
  symlinkTarget?: string | null
  rejectFeedback?: string
  acceptFeedback?: string
  setFocusedOption?: (value: string) => void
  onInputModeToggle?: (value: string) => void
  focusedOption?: string
  yesInputMode?: boolean
  noInputMode?: boolean
  /** Prefix added to resolve `filePath` to a base name. */
  cwd?: string
  /** When true, render a "Save file to continue…" hint. */
  isVscodeTerminal?: boolean
}

function baseName(path: string): string {
  const m = /[^/\\]+$/.exec(path)
  return m ? m[0] : path
}

function symlinkWarning(cwd: string | undefined, target: string): string {
  if (cwd && target.startsWith(cwd)) {
    return `Symlink target: ${target}`
  }
  return `This will modify ${target} (outside working directory) via a symlink`
}

export function ShowInIDEPrompt<A>({
  filePath,
  input,
  onChange,
  options,
  ideName,
  symlinkTarget,
  rejectFeedback = '',
  acceptFeedback = '',
  setFocusedOption,
  focusedOption,
  yesInputMode = false,
  noInputMode = false,
  cwd,
  isVscodeTerminal = false,
}: Props<A>) {
  const [selected, setSelected] = useState(() =>
    Math.max(0, options.findIndex(o => o.value === focusedOption)),
  )

  const safeIndex = Math.max(
    0,
    Math.min(options.length - 1, Number.isNaN(selected) ? 0 : selected),
  )

  const commit = (idx: number) => {
    const opt = options[idx]
    if (!opt) return
    if (opt.option.type === 'reject') {
      const trimmed = rejectFeedback.trim()
      onChange(opt.option, input, trimmed || undefined)
      return
    }
    if (opt.option.type === 'accept-once') {
      const trimmed = acceptFeedback.trim()
      onChange(opt.option, input, trimmed || undefined)
      return
    }
    onChange(opt.option, input)
  }

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const input = (seq ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input) {
      const match = options.findIndex(
        o => o.hotkey && o.hotkey.toLowerCase() === input,
      )
      if (match >= 0) {
        commit(match)
        return
      }
    }
    if (name === 'escape') {
      const rej = options.findIndex(o => o.option.type === 'reject')
      commit(rej >= 0 ? rej : safeIndex)
      return
    }
    if (name === 'up' || input === 'k') {
      const next = Math.max(0, safeIndex - 1)
      setSelected(next)
      setFocusedOption?.(options[next]!.value)
      return
    }
    if (name === 'down' || input === 'j') {
      const next = Math.min(options.length - 1, safeIndex + 1)
      setSelected(next)
      setFocusedOption?.(options[next]!.value)
      return
    }
    if (name === 'return' || name === 'enter') {
      commit(safeIndex)
    }
  })

  const focused = options[safeIndex]
  const showTabHint =
    (focused?.value === 'yes' && !yesInputMode) ||
    (focused?.value === 'no' && !noInputMode)

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
    >
      <strong>
        <text fg={c.warning}>Opened changes in {ideName} \u29C9</text>
      </strong>

      {symlinkTarget && (
        <box marginTop={1}>
          <text fg={c.warning}>{symlinkWarning(cwd, symlinkTarget)}</text>
        </box>
      )}

      {isVscodeTerminal && (
        <box marginTop={1}>
          <text fg={c.dim}>Save file to continue…</text>
        </box>
      )}

      <box marginTop={1} flexDirection="column">
        <text>
          Do you want to make this edit to{' '}
          <strong>{baseName(filePath)}</strong>?
        </text>
        {options.map((opt, i) => {
          const isSelected = i === safeIndex
          return (
            <box key={opt.value} flexDirection="row">
              <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                <strong>{` ${opt.label} `}</strong>
              </text>
              {opt.hotkey && <text fg={c.dim}> ({opt.hotkey})</text>}
            </box>
          )
        })}
      </box>

      <box marginTop={1}>
        <text fg={c.dim}>
          Esc to cancel{showTabHint ? ' · Tab to amend' : ''}
        </text>
      </box>
    </box>
  )
}
