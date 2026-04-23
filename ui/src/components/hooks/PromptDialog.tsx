import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import type { PromptHookRequest } from './types.js'

/**
 * Interactive prompt spawned by a prompt-type hook (e.g. a tool-call
 * confirmation routed through a hook).
 *
 * OpenTUI-native port of the upstream `hooks/PromptDialog`
 * (`ui/examples/upstream-patterns/src/components/hooks/PromptDialog.tsx`).
 * Upstream wrapped `<PermissionDialog>` + `<Select>`; the Lite port
 * renders the same list of labelled options and respects `app:interrupt`
 * (Ctrl+C) by calling `onAbort`.
 */

type Props = {
  title: string
  toolInputSummary?: string | null
  request: PromptHookRequest
  onRespond: (key: string) => void
  onAbort: () => void
}

export function PromptDialog({
  title,
  toolInputSummary,
  request,
  onRespond,
  onAbort,
}: Props) {
  const options = request.options
  const [cursor, setCursor] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = event.sequence ?? name ?? ''
    if (event.ctrl && input === 'c') {
      onAbort()
      return
    }
    if (name === 'escape') {
      onAbort()
      return
    }
    if (options.length === 0) return
    if (name === 'up' || input === 'k') {
      setCursor(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setCursor(prev => Math.min(options.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const selected = options[cursor]
      if (selected) onRespond(selected.key)
      return
    }
    if (input && input.length === 1) {
      const match = options.findIndex(opt => opt.key === input)
      if (match >= 0) {
        onRespond(options[match]!.key)
      }
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
      title={title}
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="row" justifyContent="space-between">
        <text>{request.title ?? 'Hook prompt'}</text>
        {toolInputSummary && <text fg={c.dim}>{toolInputSummary}</text>}
      </box>
      <box marginTop={1}>
        <text>{request.message}</text>
      </box>
      <box marginTop={1} flexDirection="column">
        {options.map((opt, i) => {
          const isSelected = i === cursor
          return (
            <box key={opt.key} flexDirection="column">
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` ${opt.label} `}</strong>
                <span fg={isSelected ? c.bg : c.dim}>{` (${opt.key})`}</span>
              </text>
              {opt.description && (
                <text fg={c.dim}>{`   ${opt.description}`}</text>
              )}
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          Up/Down to move \u00B7 Enter to select \u00B7 hotkey matches \u00B7
          Ctrl+C to abort
        </text>
      </box>
    </box>
  )
}
