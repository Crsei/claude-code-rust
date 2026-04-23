import React, { useCallback, useEffect, useMemo, useRef } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/ExitFlow.tsx`.
 *
 * Upstream orchestrates the goodbye banner + optional `WorktreeExitDialog`
 * and calls `gracefulShutdown(0, 'prompt_input_exit')`. cc-rust does not
 * ship the upstream worktree tooling, so this Lite variant focuses on
 * the two jobs the active frontend actually needs:
 *
 *  1. Show a confirmation prompt when the user hits `Ctrl+D` / `/exit`
 *     inside a git worktree (`showWorktree = true`). The body carries a
 *     short reminder that worktree state won't be merged automatically.
 *  2. Otherwise emit the random goodbye string and hand control back to
 *     the caller — they drive `process.exit` via the IPC `quit` handler
 *     in `main.tsx`.
 *
 * Callers pass an `onDone` which receives the goodbye message, and an
 * optional `onCancel` for the worktree dialog's Esc handler.
 */

const GOODBYE_MESSAGES = [
  'Goodbye!',
  'See ya!',
  'Bye!',
  'Catch you later!',
] as const

function pickGoodbye(): string {
  const index = Math.floor(Math.random() * GOODBYE_MESSAGES.length)
  return GOODBYE_MESSAGES[index] ?? 'Goodbye!'
}

type Props = {
  showWorktree: boolean
  onDone: (message?: string) => void
  onCancel?: () => void
}

export function ExitFlow({ showWorktree, onDone, onCancel }: Props) {
  const onDoneRef = useRef(onDone)
  useEffect(() => {
    onDoneRef.current = onDone
  })

  const finish = useCallback((message?: string) => {
    onDoneRef.current(message ?? pickGoodbye())
  }, [])

  // No dialog needed — just emit the goodbye and exit. We run inside a
  // `useEffect` so the caller sees exactly one invocation even if React
  // strict-mode double-mounts the component.
  useEffect(() => {
    if (!showWorktree) finish()
  }, [showWorktree, finish])

  if (!showWorktree) return null

  return <WorktreeExitDialog onDone={finish} onCancel={onCancel} />
}

/**
 * Worktree exit confirmation. Lite-native replacement for upstream's
 * `WorktreeExitDialog` — cc-rust doesn't ship the upstream dialog, so
 * we inline a minimal two-button prompt here (Enter = exit, Esc =
 * cancel). Mirrors the layout used by `LspRecommendationDialog` so the
 * visual language stays consistent.
 */
function WorktreeExitDialog({
  onDone,
  onCancel,
}: {
  onDone: (message?: string) => void
  onCancel?: () => void
}) {
  const cancel = useCallback(() => {
    onCancel?.()
  }, [onCancel])

  const options = useMemo(
    () => [
      { label: 'Exit worktree', hotkey: 'y' },
      { label: 'Cancel', hotkey: 'n' },
    ],
    [],
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? name ?? '').toLowerCase()
    if (name === 'escape' || input === 'n') {
      cancel()
      return
    }
    if (name === 'return' || name === 'enter' || input === 'y') {
      onDone()
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
      title="Exit worktree?"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        <span fg={c.dim}>
          Unmerged changes in this worktree will stay local. Merge or commit
          before leaving if you intend to keep them.
        </span>
      </text>
      <box marginTop={1} flexDirection="row" gap={2}>
        {options.map(opt => (
          <text key={opt.label} fg={c.textBright}>
            <strong>{` ${opt.label} `}</strong>
            <span fg={c.dim}> ({opt.hotkey})</span>
          </text>
        ))}
      </box>
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Enter to exit · Esc or `n` to cancel
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}
