import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/RemoteCallout.tsx`.
 *
 * First-time prompt that asks whether to enable Remote Control (the
 * claude.ai/code bridge). Upstream also writes `remoteDialogSeen` to
 * the global config on mount — the Lite build surfaces that side-effect
 * through the optional `onSeen` callback so the caller can persist.
 */

export type RemoteCalloutSelection = 'enable' | 'dismiss'

type Option = {
  value: RemoteCalloutSelection
  label: string
  description: string
  hotkey: string
}

const OPTIONS: Option[] = [
  {
    value: 'enable',
    label: 'Enable Remote Control for this session',
    description: 'Opens a secure connection to claude.ai.',
    hotkey: 'y',
  },
  {
    value: 'dismiss',
    label: 'Never mind',
    description: 'You can always enable it later with /remote-control.',
    hotkey: 'n',
  },
]

type Props = {
  onDone: (selection: RemoteCalloutSelection) => void
  /** Invoked once when the callout first mounts so the caller can
   *  persist `remoteDialogSeen` (upstream wrote directly to config). */
  onSeen?: () => void
}

export function RemoteCallout({ onDone, onSeen }: Props) {
  const [selected, setSelected] = useState(0)

  React.useEffect(() => {
    onSeen?.()
  }, [onSeen])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      onDone('dismiss')
      return
    }
    if (key) {
      const hit = OPTIONS.findIndex(o => o.hotkey === key)
      if (hit >= 0) {
        onDone(OPTIONS[hit]!.value)
        return
      }
    }
    if (name === 'up' || key === 'k') {
      setSelected(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setSelected(idx => Math.min(OPTIONS.length - 1, idx + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      onDone(OPTIONS[selected]!.value)
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
      paddingX={2}
      paddingY={1}
    >
      <strong>
        <text fg={c.warning}>Remote Control</text>
      </strong>

      <box flexDirection="column" marginTop={1}>
        <text>
          Remote Control lets you access this CLI session from the web
          (claude.ai/code) or the Claude app, so you can pick up where you
          left off on any device.
        </text>
        <text> </text>
        <text>
          You can disconnect remote access anytime by running /remote-control
          again.
        </text>
      </box>

      <box flexDirection="column" marginTop={1}>
        {OPTIONS.map((opt, i) => {
          const isSelected = i === selected
          return (
            <box key={opt.value} flexDirection="column">
              <box flexDirection="row">
                <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                  <strong>{` ${opt.label} `}</strong>
                </text>
                <text fg={c.dim}> ({opt.hotkey})</text>
              </box>
              <box paddingLeft={3}>
                <text fg={c.dim}>{opt.description}</text>
              </box>
            </box>
          )
        })}
      </box>

      <box marginTop={1}>
        <text fg={c.dim}>Enter to confirm · Esc to dismiss</text>
      </box>
    </box>
  )
}
