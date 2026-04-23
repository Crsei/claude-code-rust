import React, { useCallback, useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { TeleportStash, type GitFileStatus } from './TeleportStash.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TeleportError.tsx`.
 *
 * Resolves preconditions before a teleport — login and a clean working
 * tree. The upstream talks directly to `checkNeedsClaudeAiLogin`,
 * `checkIsGitClean`, and `ConsoleOAuthFlow`. The Rust port accepts those
 * as injected callbacks so the same shape can be wired from the Rust
 * daemon or stubbed out in tests. The OAuth handoff is represented by a
 * caller-owned `renderLoginFlow` thunk — upstream uses Ink's
 * `ConsoleOAuthFlow` component there, and the Rust frontend owns its
 * own OAuth UI.
 */

export type TeleportLocalErrorType = 'needsLogin' | 'needsGitStash'

type Props = {
  onComplete: () => void
  errorsToIgnore?: ReadonlySet<TeleportLocalErrorType>
  /** Returns `true` when the user isn't logged in via claude.ai yet. */
  checkNeedsClaudeAiLogin: () => Promise<boolean>
  /** Returns `true` when `git status` reports a clean working tree. */
  checkIsGitClean: () => Promise<boolean>
  /** Exit flow (upstream calls `gracefulShutdownSync(0)`). */
  onExit: () => void
  /** Render a login flow. `onDone` is wired to continue the teleport. */
  renderLoginFlow: (props: { onDone: () => void }) => React.ReactElement
  /** Git helpers for the nested `<TeleportStash>` form. */
  getFileStatus: () => Promise<GitFileStatus>
  stashToCleanState: (message: string) => Promise<boolean>
}

const EMPTY_ERRORS_TO_IGNORE: ReadonlySet<TeleportLocalErrorType> = new Set()
const LOGIN_OPTIONS = [
  { label: 'Login with Claude account', value: 'login' },
  { label: 'Exit', value: 'exit' },
] as const

export function TeleportError({
  onComplete,
  errorsToIgnore = EMPTY_ERRORS_TO_IGNORE,
  checkNeedsClaudeAiLogin,
  checkIsGitClean,
  onExit,
  renderLoginFlow,
  getFileStatus,
  stashToCleanState,
}: Props): React.ReactElement | null {
  const [currentError, setCurrentError] =
    useState<TeleportLocalErrorType | null>(null)
  const [isLoggingIn, setIsLoggingIn] = useState(false)
  const [loginFocusIndex, setLoginFocusIndex] = useState(0)

  const checkErrors = useCallback(async () => {
    const [needsLogin, isClean] = await Promise.all([
      checkNeedsClaudeAiLogin(),
      checkIsGitClean(),
    ])
    const errors = new Set<TeleportLocalErrorType>()
    if (needsLogin && !errorsToIgnore.has('needsLogin')) {
      errors.add('needsLogin')
    }
    if (!isClean && !errorsToIgnore.has('needsGitStash')) {
      errors.add('needsGitStash')
    }
    if (errors.size === 0) {
      onComplete()
      return
    }
    if (errors.has('needsLogin')) {
      setCurrentError('needsLogin')
    } else if (errors.has('needsGitStash')) {
      setCurrentError('needsGitStash')
    }
  }, [
    checkIsGitClean,
    checkNeedsClaudeAiLogin,
    errorsToIgnore,
    onComplete,
  ])

  useEffect(() => {
    void checkErrors()
  }, [checkErrors])

  const handleLoginComplete = useCallback(() => {
    setIsLoggingIn(false)
    void checkErrors()
  }, [checkErrors])

  const handleLoginDialogSelect = useCallback(
    (value: string) => {
      if (value === 'login') {
        setIsLoggingIn(true)
        return
      }
      onExit()
    },
    [onExit],
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (currentError !== 'needsLogin' || isLoggingIn) return
    const name = event.name
    if (name === 'escape') {
      onExit()
      return
    }
    if (name === 'up') {
      setLoginFocusIndex(i => (i - 1 + LOGIN_OPTIONS.length) % LOGIN_OPTIONS.length)
      return
    }
    if (name === 'down' || name === 'tab') {
      setLoginFocusIndex(i => (i + 1) % LOGIN_OPTIONS.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      handleLoginDialogSelect(LOGIN_OPTIONS[loginFocusIndex]!.value)
    }
  })

  if (!currentError) return null

  if (currentError === 'needsGitStash') {
    return (
      <TeleportStash
        onStashAndContinue={() => void checkErrors()}
        onCancel={onExit}
        getFileStatus={getFileStatus}
        stashToCleanState={stashToCleanState}
      />
    )
  }

  if (isLoggingIn) {
    return renderLoginFlow({ onDone: handleLoginComplete })
  }

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
      title="Log in to Claude"
      titleAlignment="center"
    >
      <box flexDirection="column">
        <text fg={c.dim}>Teleport requires a Claude.ai account.</text>
        <text fg={c.dim}>
          Your Claude Pro/Max subscription will be used by Claude Code.
        </text>
      </box>
      <box marginTop={1} flexDirection="column">
        {LOGIN_OPTIONS.map((opt, i) => {
          const isFocused = i === loginFocusIndex
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
  )
}

/**
 * Collects the current set of teleport errors. Kept exported so callers
 * can reuse the same check when they need to decide whether to surface
 * the dialog at all.
 */
export async function getTeleportErrors(
  checkNeedsClaudeAiLogin: () => Promise<boolean>,
  checkIsGitClean: () => Promise<boolean>,
): Promise<Set<TeleportLocalErrorType>> {
  const errors = new Set<TeleportLocalErrorType>()
  const [needsLogin, isClean] = await Promise.all([
    checkNeedsClaudeAiLogin(),
    checkIsGitClean(),
  ])
  if (needsLogin) errors.add('needsLogin')
  if (!isClean) errors.add('needsGitStash')
  return errors
}
