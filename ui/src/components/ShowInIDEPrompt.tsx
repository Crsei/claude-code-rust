import { basename, relative } from 'path'
import React from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/ShowInIDEPrompt.tsx`.
 *
 * Shown while the IDE has the proposed file edit open in a save-to-apply
 * flow. Accept / reject actions bubble back through `onChange`, and the
 * caller owns the feedback strings for both outcomes. The upstream uses
 * Ink's `Pane` + `Select` pair; here we use an OpenTUI box frame and a
 * keyboard-driven two-option picker so accept/reject still work without
 * a mouse.
 *
 * The `options` prop remains `PermissionOptionWithLabel[]` (the sample
 * tree's type) so callers can wire this into the same permission-menu
 * machinery. The Rust port doesn't model the `accept-once` vs
 * `accept-always` distinction in the frontend — the backend drives that
 * choice — so accepted clicks always fire `onChange` with whichever
 * option the user is focusing.
 */

export type PermissionOption =
  | { type: 'reject' }
  | { type: 'accept-once' }
  | { type: 'accept-always' }

export type PermissionOptionWithLabel = {
  option: PermissionOption
  value: string
  label: string
  description?: string
}

type Props<A> = {
  filePath: string
  input: A
  onChange: (option: PermissionOption, args: A, feedback?: string) => void
  options: PermissionOptionWithLabel[]
  ideName: string
  symlinkTarget?: string | null
  rejectFeedback: string
  acceptFeedback: string
  setFocusedOption: (value: string) => void
  onInputModeToggle: (value: string) => void
  focusedOption: string
  yesInputMode: boolean
  noInputMode: boolean
  /** Used to resolve relative symlink messages. Defaults to `process.cwd()`. */
  cwd?: string
  /** Defaults to `true` — upstream derives this from the terminal sniffer. */
  showSaveHint?: boolean
}

export function ShowInIDEPrompt<A>({
  onChange,
  options,
  input,
  filePath,
  ideName,
  symlinkTarget,
  rejectFeedback,
  acceptFeedback,
  setFocusedOption,
  onInputModeToggle,
  focusedOption,
  yesInputMode,
  noInputMode,
  cwd,
  showSaveHint = true,
}: Props<A>): React.ReactElement {
  const focusIndex = Math.max(
    0,
    options.findIndex(opt => opt.value === focusedOption),
  )

  const selectOption = (opt: PermissionOptionWithLabel) => {
    if (opt.option.type === 'reject') {
      const trimmed = rejectFeedback.trim()
      onChange(opt.option, input, trimmed.length > 0 ? trimmed : undefined)
      return
    }
    if (opt.option.type === 'accept-once') {
      const trimmed = acceptFeedback.trim()
      onChange(opt.option, input, trimmed.length > 0 ? trimmed : undefined)
      return
    }
    onChange(opt.option, input)
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name

    if (name === 'escape') {
      onChange({ type: 'reject' }, input)
      return
    }

    if (name === 'up') {
      const prev =
        options[(focusIndex - 1 + options.length) % options.length]!
      setFocusedOption(prev.value)
      return
    }
    if (name === 'down') {
      const next = options[(focusIndex + 1) % options.length]!
      setFocusedOption(next.value)
      return
    }
    if (name === 'tab') {
      onInputModeToggle(options[focusIndex]!.value)
      return
    }
    if (name === 'return' || name === 'enter') {
      const opt = options[focusIndex]
      if (opt) selectOption(opt)
    }
  })

  const effectiveCwd = cwd ?? process.cwd()
  const symlinkLine =
    symlinkTarget
      ? relative(effectiveCwd, symlinkTarget).startsWith('..')
        ? `This will modify ${symlinkTarget} (outside working directory) via a symlink`
        : `Symlink target: ${symlinkTarget}`
      : null

  const showTabHint =
    (focusedOption === 'yes' && !yesInputMode) ||
    (focusedOption === 'no' && !noInputMode)

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
    >
      <text>
        <strong>
          <span fg={c.warning}>Opened changes in {ideName} \u29C9</span>
        </strong>
      </text>

      {symlinkLine && (
        <box marginTop={1}>
          <text fg={c.warning}>{symlinkLine}</text>
        </box>
      )}

      {showSaveHint && (
        <box marginTop={1}>
          <text fg={c.dim}>Save file to continue\u2026</text>
        </box>
      )}

      <box marginTop={1} flexDirection="column">
        <text>
          Do you want to make this edit to{' '}
          <strong>{basename(filePath)}</strong>?
        </text>
        <box marginTop={1} flexDirection="column">
          {options.map((opt, i) => {
            const isSelected = i === focusIndex
            return (
              <box key={opt.value} flexDirection="row">
                <text
                  fg={isSelected ? c.bg : c.text}
                  bg={isSelected ? c.textBright : undefined}
                >
                  {` ${opt.label} `}
                </text>
                {opt.description ? (
                  <text fg={c.dim}>{'  '}{opt.description}</text>
                ) : null}
              </box>
            )
          })}
        </box>
      </box>

      <box marginTop={1}>
        <text fg={c.dim}>
          Esc to cancel{showTabHint ? ' \u00b7 Tab to amend' : ''}
        </text>
      </box>
    </box>
  )
}
