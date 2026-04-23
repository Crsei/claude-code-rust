import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TeleportStash.tsx`.
 *
 * Shown by the teleport flow when the working tree isn't clean. The
 * user can stash and continue or bail out. Upstream talks directly to
 * `getFileStatus` / `stashToCleanState` / `logForDebugging`; the Rust
 * port accepts all three as injected callbacks so the component is easy
 * to port to any backend wiring and to test in isolation.
 */

export type GitFileStatus = {
  tracked: string[]
  untracked: string[]
}

type Props = {
  onStashAndContinue: () => void
  onCancel: () => void
  /** Fetch the current git status of the working tree. */
  getFileStatus: () => Promise<GitFileStatus>
  /** Run `git stash push -m <message>` and return `true` on success. */
  stashToCleanState: (message: string) => Promise<boolean>
  /** Optional logger — upstream wires this to `logForDebugging`. */
  logForDebugging?: (message: string, meta?: { level: 'error' | 'warn' | 'info' }) => void
}

const STASH_VALUE = 'stash'
const EXIT_VALUE = 'exit'
const OPTIONS = [
  { label: 'Stash changes and continue', value: STASH_VALUE },
  { label: 'Exit', value: EXIT_VALUE },
] as const

export function TeleportStash({
  onStashAndContinue,
  onCancel,
  getFileStatus,
  stashToCleanState,
  logForDebugging,
}: Props): React.ReactElement {
  const [gitFileStatus, setGitFileStatus] = useState<GitFileStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [stashing, setStashing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [focusIndex, setFocusIndex] = useState(0)

  const changedFiles = gitFileStatus
    ? [...gitFileStatus.tracked, ...gitFileStatus.untracked]
    : []

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const status = await getFileStatus()
        if (cancelled) return
        setGitFileStatus(status)
      } catch (err) {
        if (cancelled) return
        const msg = err instanceof Error ? err.message : String(err)
        logForDebugging?.(`Error getting changed files: ${msg}`, {
          level: 'error',
        })
        setError('Failed to get changed files')
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [getFileStatus, logForDebugging])

  const handleStash = async () => {
    setStashing(true)
    try {
      logForDebugging?.('Stashing changes before teleport...')
      const ok = await stashToCleanState('Teleport auto-stash')
      if (ok) {
        logForDebugging?.('Successfully stashed changes')
        onStashAndContinue()
      } else {
        setError('Failed to stash changes')
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      logForDebugging?.(`Error stashing changes: ${msg}`, { level: 'error' })
      setError('Failed to stash changes')
    } finally {
      setStashing(false)
    }
  }

  const handleSelectChange = (value: string) => {
    if (value === STASH_VALUE) {
      void handleStash()
      return
    }
    onCancel()
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (loading || stashing) return
    if (error) return
    const name = event.name
    if (name === 'escape') {
      onCancel()
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
      handleSelectChange(OPTIONS[focusIndex]!.value)
    }
  })

  if (loading) {
    return (
      <box flexDirection="column" paddingX={1} paddingY={1}>
        <box flexDirection="row" marginBottom={1}>
          <Spinner label="Checking git status\u2026" />
        </box>
      </box>
    )
  }

  if (error) {
    return (
      <box flexDirection="column" paddingX={1} paddingY={1}>
        <text fg={c.error}>
          <strong>Error: {error}</strong>
        </text>
        <box marginTop={1} flexDirection="row">
          <text fg={c.dim}>Press </text>
          <text>
            <strong>Escape</strong>
          </text>
          <text fg={c.dim}> to cancel</text>
        </box>
      </box>
    )
  }

  const showFileCount = changedFiles.length > 8

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
      title="Working Directory Has Changes"
      titleAlignment="center"
    >
      <text>
        Teleport will switch git branches. The following changes were found:
      </text>

      <box flexDirection="column" paddingLeft={2} marginTop={1}>
        {changedFiles.length > 0 ? (
          showFileCount ? (
            <text>{changedFiles.length} files changed</text>
          ) : (
            changedFiles.map((file, index) => (
              <text key={index}>{file}</text>
            ))
          )
        ) : (
          <text fg={c.dim}>No changes detected</text>
        )}
      </box>

      <box marginTop={1}>
        <text>
          Would you like to stash these changes and continue with teleport?
        </text>
      </box>

      {stashing ? (
        <box marginTop={1} flexDirection="row">
          <Spinner label="Stashing changes..." />
        </box>
      ) : (
        <box marginTop={1} flexDirection="column">
          {OPTIONS.map((opt, i) => {
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
      )}
    </box>
  )
}
