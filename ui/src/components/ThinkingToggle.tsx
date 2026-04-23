import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/ThinkingToggle.tsx`.
 *
 * Upstream uses Ink's `Pane`, `Select`, `Byline`, and the `useKeybinding`
 * + `useExitOnCtrlCDWithKeybindings` helpers. The Rust port keeps the
 * same two-option layout and mid-conversation confirmation behaviour
 * using OpenTUI intrinsics and a raw `useKeyboard` listener.
 *
 * Behaviour summary:
 *  - Pick `Enabled` / `Disabled` to toggle thinking for the session.
 *  - If `isMidConversation` is true AND the selection differs from
 *    `currentValue`, a warning confirmation step shows first.
 *  - Esc from the confirmation step returns to the select; Esc from the
 *    select fires `onCancel`.
 */

type Option = {
  value: 'true' | 'false'
  label: string
  description: string
}

const OPTIONS: Option[] = [
  {
    value: 'true',
    label: 'Enabled',
    description: 'Claude will think before responding',
  },
  {
    value: 'false',
    label: 'Disabled',
    description: 'Claude will respond without extended thinking',
  },
]

export type Props = {
  currentValue: boolean
  onSelect: (enabled: boolean) => void
  onCancel?: () => void
  isMidConversation?: boolean
}

export function ThinkingToggle({
  currentValue,
  onSelect,
  onCancel,
  isMidConversation,
}: Props): React.ReactElement {
  const [focusIndex, setFocusIndex] = useState(
    OPTIONS.findIndex(o => o.value === (currentValue ? 'true' : 'false')),
  )
  const [confirmationPending, setConfirmationPending] = useState<
    boolean | null
  >(null)

  const confirm = (chosen: boolean) => {
    if (isMidConversation && chosen !== currentValue) {
      setConfirmationPending(chosen)
      return
    }
    onSelect(chosen)
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name

    if (confirmationPending !== null) {
      if (name === 'escape') {
        setConfirmationPending(null)
        return
      }
      if (name === 'return' || name === 'enter') {
        onSelect(confirmationPending)
      }
      return
    }

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'up') {
      setFocusIndex(i => (i - 1 + OPTIONS.length) % OPTIONS.length)
      return
    }
    if (name === 'down' || name === 'tab') {
      setFocusIndex(i => (i + 1) % OPTIONS.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      const choice = OPTIONS[focusIndex]
      if (choice) confirm(choice.value === 'true')
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
    >
      <box marginBottom={1} flexDirection="column">
        <text>
          <strong>
            <span fg={c.warning}>Toggle thinking mode</span>
          </strong>
        </text>
        <text fg={c.dim}>Enable or disable thinking for this session.</text>
      </box>

      {confirmationPending !== null ? (
        <box flexDirection="column" marginBottom={1}>
          <text fg={c.warning}>
            Changing thinking mode mid-conversation will increase latency and
            may reduce quality. For best results, set this at the start of a
            session.
          </text>
          <box marginTop={1}>
            <text fg={c.warning}>Do you want to proceed?</text>
          </box>
        </box>
      ) : (
        <box flexDirection="column" marginBottom={1}>
          {OPTIONS.map((opt, i) => {
            const isFocused = i === focusIndex
            return (
              <box key={opt.value} flexDirection="column">
                <text
                  fg={isFocused ? c.bg : c.text}
                  bg={isFocused ? c.textBright : undefined}
                >
                  {isFocused ? '\u25B8 ' : '  '}
                  {opt.label}
                </text>
                <text fg={c.dim}>    {opt.description}</text>
              </box>
            )
          })}
        </box>
      )}

      <text fg={c.dim}>
        <em>
          {confirmationPending !== null
            ? 'Enter to confirm \u00b7 Esc to cancel'
            : 'Enter to confirm \u00b7 Esc to exit'}
        </em>
      </text>
    </box>
  )
}
