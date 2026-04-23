import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { StructuredDiff } from './StructuredDiff.js'
import type { DiffHunk } from './StructuredDiff/hunks.js'

/**
 * Ported from `ui/examples/upstream-patterns/src/components/ThemePicker.tsx`.
 *
 * Upstream's picker drives Ink's `usePreviewTheme` / `useThemeSetting`
 * hooks to swap the active theme live as the user navigates. The Rust
 * port doesn't own a theme registry yet (only a single palette in
 * `theme.ts`), so this port exposes the *shape* of the upstream picker
 * — full option list, navigation, preview diff, syntax-highlight hint —
 * and delegates the actual theme swap to callbacks the caller wires up:
 *
 *  - `onPreview(setting)` fires as the focused option changes.
 *  - `onSelect(setting)` fires on Enter.
 *  - `onCancel()` fires on Esc (or when the caller wants `hideEscToCancel`
 *    to drive the shortcut bar but still allow cancel externally).
 *
 * Options mirror the upstream list one-for-one so future theme work can
 * fan in without rewriting this component.
 */

export type ThemeSetting =
  | 'auto'
  | 'dark'
  | 'light'
  | 'dark-daltonized'
  | 'light-daltonized'
  | 'dark-ansi'
  | 'light-ansi'

type ThemeOption = {
  label: string
  value: ThemeSetting
}

const AUTO_OPTION: ThemeOption = { label: 'Auto (match terminal)', value: 'auto' }

const BASE_OPTIONS: ThemeOption[] = [
  { label: 'Dark mode', value: 'dark' },
  { label: 'Light mode', value: 'light' },
  { label: 'Dark mode (colorblind-friendly)', value: 'dark-daltonized' },
  { label: 'Light mode (colorblind-friendly)', value: 'light-daltonized' },
  { label: 'Dark mode (ANSI colors only)', value: 'dark-ansi' },
  { label: 'Light mode (ANSI colors only)', value: 'light-ansi' },
]

const PREVIEW_HUNK: DiffHunk = {
  oldStart: 1,
  newStart: 1,
  oldLines: 3,
  newLines: 3,
  lines: [
    { kind: 'context', text: 'function greet() {' },
    { kind: 'remove', text: '  console.log("Hello, World!");' },
    { kind: 'add', text: '  console.log("Hello, Claude!");' },
    { kind: 'context', text: '}' },
  ],
}

export type ThemePickerProps = {
  currentTheme: ThemeSetting
  onSelect: (setting: ThemeSetting) => void
  onCancel?: () => void
  onPreview?: (setting: ThemeSetting) => void
  showIntroText?: boolean
  helpText?: string
  showHelpTextBelow?: boolean
  hideEscToCancel?: boolean
  /** When true, the `Auto` option is included at the top of the list. */
  includeAuto?: boolean
  /** Syntax highlighting state label — rendered beneath the preview. */
  syntaxStatusLabel?: string
}

export function ThemePicker({
  currentTheme,
  onSelect,
  onCancel,
  onPreview,
  showIntroText = false,
  helpText = '',
  showHelpTextBelow = false,
  hideEscToCancel = false,
  includeAuto = false,
  syntaxStatusLabel,
}: ThemePickerProps): React.ReactElement {
  const options = includeAuto
    ? [AUTO_OPTION, ...BASE_OPTIONS]
    : BASE_OPTIONS

  const [focusIndex, setFocusIndex] = useState(() => {
    const i = options.findIndex(o => o.value === currentTheme)
    return i >= 0 ? i : 0
  })

  const moveFocus = (next: number) => {
    setFocusIndex(next)
    const candidate = options[next]
    if (candidate) onPreview?.(candidate.value)
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'up') {
      moveFocus((focusIndex - 1 + options.length) % options.length)
      return
    }
    if (name === 'down' || name === 'tab') {
      moveFocus((focusIndex + 1) % options.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      const choice = options[focusIndex]
      if (choice) onSelect(choice.value)
    }
  })

  const content = (
    <box flexDirection="column">
      <box flexDirection="column">
        {showIntroText ? (
          <text>Let&apos;s get started.</text>
        ) : (
          <text>
            <strong>
              <span fg={c.warning}>Theme</span>
            </strong>
          </text>
        )}
        <box flexDirection="column" marginTop={1}>
          <text>
            <strong>
              Choose the text style that looks best with your terminal
            </strong>
          </text>
          {helpText && !showHelpTextBelow ? (
            <text fg={c.dim}>{helpText}</text>
          ) : null}
        </box>
        <box marginTop={1} flexDirection="column">
          {options.map((opt, i) => {
            const isFocused = i === focusIndex
            return (
              <text
                key={opt.value}
                fg={isFocused ? c.bg : c.text}
                bg={isFocused ? c.textBright : undefined}
              >
                {isFocused ? '\u25B8 ' : '  '}
                {opt.label}
              </text>
            )
          })}
        </box>
      </box>

      <box
        marginTop={1}
        flexDirection="column"
        borderStyle="single"
        borderColor={c.dim}
      >
        <StructuredDiff hunk={PREVIEW_HUNK} filePath="demo.js" />
      </box>
      {syntaxStatusLabel ? (
        <text fg={c.dim}> {syntaxStatusLabel}</text>
      ) : null}
    </box>
  )

  if (!showIntroText) {
    return (
      <>
        <box flexDirection="column">{content}</box>
        <box marginTop={1} flexDirection="column">
          {showHelpTextBelow && helpText ? (
            <box marginLeft={3}>
              <text fg={c.dim}>{helpText}</text>
            </box>
          ) : null}
          {!hideEscToCancel ? (
            <text fg={c.dim}>
              <em>Enter to select \u00b7 Esc to cancel</em>
            </text>
          ) : null}
        </box>
      </>
    )
  }

  return content
}
