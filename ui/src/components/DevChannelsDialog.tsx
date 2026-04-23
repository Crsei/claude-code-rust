import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/DevChannelsDialog.tsx`.
 *
 * Upstream gates the `--dangerously-load-development-channels` CLI
 * flag behind an explicit "I am using this for local development"
 * confirmation. cc-rust's channel loader runs the same gate, so we
 * mirror the copy and the accept/exit Select path. Exit is surfaced as
 * a callback — callers decide how to tear down (cc-rust's `main.tsx`
 * listens for the `exit` IPC event).
 *
 * `ChannelEntry` is a local-only shape: the backend ships a `name`
 * and a discriminant for "plugin loaded from marketplace" vs "bare
 * channel server". Keeping the type here instead of importing from
 * `ipc/protocol.ts` keeps the dialog reusable — the CLI surface that
 * needs this dialog also wants to display an arbitrary channel list
 * that hasn't yet been fed through the IPC.
 */

export type ChannelEntry =
  | { kind: 'plugin'; name: string; marketplace: string }
  | { kind: 'server'; name: string }

type Props = {
  channels: ChannelEntry[]
  onAccept: () => void
  /** Called when the user explicitly picks "Exit" or hits Esc. */
  onExit: (code: 0 | 1) => void
}

type Decision = 'accept' | 'exit'

const OPTIONS: Array<{ value: Decision; label: string; hotkey: string }> = [
  { value: 'accept', label: 'I am using this for local development', hotkey: 'y' },
  { value: 'exit', label: 'Exit', hotkey: 'n' },
]

function formatChannel(entry: ChannelEntry): string {
  if (entry.kind === 'plugin') {
    return `plugin:${entry.name}@${entry.marketplace}`
  }
  return `server:${entry.name}`
}

export function DevChannelsDialog({ channels, onAccept, onExit }: Props) {
  const [selected, setSelected] = useState(0)
  const options = OPTIONS
  const safeIndex = Math.max(0, Math.min(selected, options.length - 1))

  const decide = (decision: Decision) => {
    if (decision === 'accept') onAccept()
    else onExit(1)
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input) {
      const match = options.findIndex(opt => opt.hotkey === input)
      if (match >= 0) {
        decide(options[match]!.value)
        return
      }
    }

    if (name === 'escape') {
      // Upstream treats Esc as a graceful 0 exit (user never saw the
      // warning, so we don't flag an error).
      onExit(0)
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
      decide(options[safeIndex]!.value)
    }
  })

  const list = channels.map(formatChannel).join(', ')

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.error}
      title="WARNING: Loading development channels"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="column" gap={1}>
        <text fg={c.text} selectable>
          --dangerously-load-development-channels is for local channel
          development only. Do not use this option to run channels you have
          downloaded off the internet.
        </text>
        <text fg={c.text} selectable>
          Please use --channels to run a list of approved channels.
        </text>
        <text fg={c.dim} selectable>
          Channels: {list || '(none declared)'}
        </text>
      </box>
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
            <span fg={c.dim}>Up/Down · Enter to choose · Esc to cancel</span>
          </em>
        </text>
      </box>
    </box>
  )
}
