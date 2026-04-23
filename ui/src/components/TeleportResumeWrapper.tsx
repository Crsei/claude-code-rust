import React, { useEffect } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TeleportResumeWrapper.tsx`.
 *
 * Wraps a caller-provided session picker with the loading + error
 * overlays teleport needs. The upstream file reaches into
 * `useTeleportResume`, `logEvent`, and Ink's `useKeybinding`; here the
 * caller supplies all three pieces through props so the component stays
 * a thin UI shell:
 *
 *  - `renderSessionPicker` (replaces `<ResumeTask>`) is expected to
 *    present the session list and call `onSelect(session)` / `onCancel()`.
 *  - `resumeState` carries the hook output (`isResuming`,
 *    `selectedSession`, `error`) from the caller's own teleport hook.
 *  - `onStart` / `onCancelled` replace `logEvent('tengu_teleport_*')`.
 */

export type TeleportResumeError = {
  message: string
  formattedMessage?: string
}

export type TeleportResumeState<Session> = {
  isResuming: boolean
  selectedSession: Session | null
  error: TeleportResumeError | null
}

type Props<Session extends { title: string }> = {
  resumeState: TeleportResumeState<Session>
  /** Fires when the user selects a session. The caller is expected to
   *  drive `resumeState` from whatever hook wraps the actual resume. */
  onSelectSession: (session: Session) => void
  onCancel: () => void
  onError?: (error: string, formattedMessage?: string) => void
  onStart?: () => void
  onCancelled?: () => void
  renderSessionPicker: (props: {
    onSelect: (session: Session) => void
    onCancel: () => void
    isEmbedded: boolean
  }) => React.ReactElement
  isEmbedded?: boolean
}

export function TeleportResumeWrapper<Session extends { title: string }>({
  resumeState,
  onSelectSession,
  onCancel,
  onError,
  onStart,
  onCancelled,
  renderSessionPicker,
  isEmbedded = false,
}: Props<Session>): React.ReactElement {
  const { isResuming, selectedSession, error } = resumeState

  useEffect(() => {
    onStart?.()
  }, [onStart])

  const handleCancel = () => {
    onCancelled?.()
    onCancel()
  }

  // Upstream's `useKeybinding('app:interrupt', ...)` only arms itself while
  // an error is visible without a caller-provided handler. Mirror that here
  // with a raw `useKeyboard` check — the parent owns its own shortcuts.
  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (!error || onError) return
    if (event.name === 'escape') {
      handleCancel()
    }
  })

  useEffect(() => {
    if (error && onError) {
      onError(error.message, error.formattedMessage)
    }
  }, [error, onError])

  if (isResuming && selectedSession) {
    return (
      <box flexDirection="column" paddingX={1} paddingY={1}>
        <box flexDirection="row">
          <Spinner label="Resuming session\u2026" />
        </box>
        <text fg={c.dim}>Loading &quot;{selectedSession.title}&quot;\u2026</text>
      </box>
    )
  }

  if (error && !onError) {
    return (
      <box flexDirection="column" paddingX={1} paddingY={1}>
        <text fg={c.error}>
          <strong>Failed to resume session</strong>
        </text>
        <text fg={c.dim}>{error.message}</text>
        <box marginTop={1}>
          <text fg={c.dim}>
            Press <strong>Esc</strong> to cancel
          </text>
        </box>
      </box>
    )
  }

  return renderSessionPicker({
    onSelect: session => onSelectSession(session),
    onCancel: handleCancel,
    isEmbedded,
  })
}
