import React, { useCallback, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TeleportRepoMismatchDialog.tsx`.
 *
 * Shown when the requested `claude --teleport` session is tied to a
 * GitHub repo that the user has multiple local checkouts of. Upstream
 * wires into `validateRepoAtPath` / `removePathFromRepo` from the native
 * teleport helper module; the Rust port takes those two as injected
 * callbacks so the component stays portable and easy to test.
 */

type Props = {
  targetRepo: string
  initialPaths: string[]
  onSelectPath: (path: string) => void
  onCancel: () => void
  /** Resolve the display form of a path (defaults to returning it verbatim). */
  getDisplayPath?: (absPath: string) => string
  /** Returns `true` when the checkout at `path` is the expected repo. */
  validateRepoAtPath?: (path: string, repo: string) => Promise<boolean>
  /** Drop a now-invalid path from the local repo mapping. */
  removePathFromRepo?: (repo: string, path: string) => void
}

const CANCEL_VALUE = '__cancel__'

export function TeleportRepoMismatchDialog({
  targetRepo,
  initialPaths,
  onSelectPath,
  onCancel,
  getDisplayPath = p => p,
  validateRepoAtPath,
  removePathFromRepo,
}: Props): React.ReactElement {
  const [availablePaths, setAvailablePaths] = useState<string[]>(initialPaths)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [validating, setValidating] = useState(false)
  const [focusIndex, setFocusIndex] = useState(0)

  const entries = availablePaths.length > 0
    ? [...availablePaths.map(p => ({ value: p, label: `Use ${getDisplayPath(p)}` })), { value: CANCEL_VALUE, label: 'Cancel' }]
    : []

  const handleChange = useCallback(
    async (value: string): Promise<void> => {
      if (value === CANCEL_VALUE) {
        onCancel()
        return
      }
      if (!validateRepoAtPath) {
        onSelectPath(value)
        return
      }
      setValidating(true)
      setErrorMessage(null)
      const isValid = await validateRepoAtPath(value, targetRepo)
      if (isValid) {
        onSelectPath(value)
        return
      }
      removePathFromRepo?.(targetRepo, value)
      setAvailablePaths(prev => prev.filter(p => p !== value))
      setValidating(false)
      setFocusIndex(0)
      setErrorMessage(
        `${getDisplayPath(value)} no longer contains the correct repository. Select another path.`,
      )
    },
    [
      getDisplayPath,
      onCancel,
      onSelectPath,
      removePathFromRepo,
      targetRepo,
      validateRepoAtPath,
    ],
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (validating) return
    const name = event.name
    if (name === 'escape') {
      onCancel()
      return
    }
    if (entries.length === 0) return
    if (name === 'up') {
      setFocusIndex(i => (i - 1 + entries.length) % entries.length)
      return
    }
    if (name === 'down' || name === 'tab') {
      setFocusIndex(i => (i + 1) % entries.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      const entry = entries[focusIndex]
      if (entry) void handleChange(entry.value)
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
      title="Teleport to Repo"
      titleAlignment="center"
      backgroundColor={c.bg}
    >
      {availablePaths.length > 0 ? (
        <>
          <box flexDirection="column">
            {errorMessage && <text fg={c.error}>{errorMessage}</text>}
            <text>
              Open Claude Code in <strong>{targetRepo}</strong>:
            </text>
          </box>

          {validating ? (
            <box marginTop={1} flexDirection="row">
              <Spinner label="Validating repository\u2026" />
            </box>
          ) : (
            <box marginTop={1} flexDirection="column">
              {entries.map((entry, i) => {
                const isFocused = i === focusIndex
                return (
                  <text
                    key={entry.value}
                    fg={isFocused ? c.bg : c.text}
                    bg={isFocused ? c.textBright : undefined}
                  >
                    {isFocused ? '\u25B8 ' : '  '}
                    {entry.label}
                  </text>
                )
              })}
            </box>
          )}
        </>
      ) : (
        <box flexDirection="column">
          {errorMessage && <text fg={c.error}>{errorMessage}</text>}
          <text fg={c.dim}>
            Run claude --teleport from a checkout of {targetRepo}
          </text>
        </box>
      )}
    </box>
  )
}
