import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/RemoteEnvironmentDialog.tsx`.
 *
 * Shows the list of teleport remote environments, the current
 * selection, and lets the user pick a new default. Upstream loads the
 * list through `getEnvironmentSelectionInfo()` and persists via
 * `updateSettingsForSource`. The Lite port accepts those handlers so
 * the caller can bind whichever backend IPC route is appropriate.
 */

export type EnvironmentResource = {
  environment_id: string
  name: string
}

export type EnvironmentSelectionInfo = {
  availableEnvironments: EnvironmentResource[]
  selectedEnvironment: EnvironmentResource | null
  /** Optional source suffix (e.g. "managed settings"), appended when
   *  the active selection isn't from local settings. */
  selectedEnvironmentSourceLabel?: string
}

type Props = {
  /** Invoked on mount to fetch the environment list. */
  loader: () => Promise<EnvironmentSelectionInfo>
  /** Persists the user's selection. */
  onSelect: (env: EnvironmentResource) => void | Promise<void>
  /** Called when the user cancels or confirms. `message` is an optional
   *  status banner the caller can echo (e.g. into the command menu). */
  onDone: (message?: string) => void
}

const DIALOG_TITLE = 'Select Remote Environment'
const SETUP_HINT = 'Configure environments at: claude.ai/code'

export function RemoteEnvironmentDialog({ loader, onSelect, onDone }: Props) {
  const [state, setState] = useState<
    | { kind: 'loading' }
    | { kind: 'ready'; info: EnvironmentSelectionInfo }
    | { kind: 'error'; message: string }
    | { kind: 'updating' }
  >({ kind: 'loading' })
  const [selectedIdx, setSelectedIdx] = useState(0)

  useEffect(() => {
    let cancelled = false
    void loader()
      .then(info => {
        if (cancelled) return
        const idx = Math.max(
          0,
          info.availableEnvironments.findIndex(
            env => env.environment_id === info.selectedEnvironment?.environment_id,
          ),
        )
        setSelectedIdx(idx)
        setState({ kind: 'ready', info })
      })
      .catch((err: unknown) => {
        if (cancelled) return
        setState({
          kind: 'error',
          message: err instanceof Error ? err.message : String(err),
        })
      })
    return () => {
      cancelled = true
    }
  }, [loader])

  const commit = (env: EnvironmentResource) => {
    setState({ kind: 'updating' })
    void Promise.resolve(onSelect(env))
      .then(() => onDone(`Set default remote environment to ${env.name} (${env.environment_id})`))
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err)
        onDone(`Error updating environment: ${message}`)
      })
  }

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      onDone()
      return
    }
    if (state.kind !== 'ready') return
    const envs = state.info.availableEnvironments
    if (envs.length === 0) {
      if (name === 'return' || name === 'enter') onDone()
      return
    }
    if (name === 'up' || key === 'k') {
      setSelectedIdx(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setSelectedIdx(idx => Math.min(envs.length - 1, idx + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const target = envs[selectedIdx]
      if (target) commit(target)
    }
  })

  const body = (() => {
    if (state.kind === 'loading') {
      return <Spinner label="Loading environments…" />
    }
    if (state.kind === 'updating') {
      return <Spinner label="Updating…" />
    }
    if (state.kind === 'error') {
      return <text fg={c.error}>Error: {state.message}</text>
    }
    const { info } = state
    const envs = info.availableEnvironments
    if (envs.length === 0 || !info.selectedEnvironment) {
      return <text>No remote environments available.</text>
    }
    if (envs.length === 1) {
      const env = envs[0]!
      return (
        <box flexDirection="column">
          <text>
            <span fg={c.success}>\u2713</span> Using <strong>{env.name}</strong>{' '}
            <span fg={c.dim}>({env.environment_id})</span>
          </text>
          <box marginTop={1}>
            <text fg={c.dim}>{SETUP_HINT}</text>
          </box>
          <box marginTop={1}>
            <text fg={c.dim}>Enter to continue · Esc to cancel</text>
          </box>
        </box>
      )
    }
    const suffix = info.selectedEnvironmentSourceLabel
      ? ` (from ${info.selectedEnvironmentSourceLabel} settings)`
      : ''
    return (
      <box flexDirection="column">
        <text>
          Currently using: <strong>{info.selectedEnvironment.name}</strong>
          {suffix}
        </text>
        <box marginTop={1}>
          <text fg={c.dim}>{SETUP_HINT}</text>
        </box>
        <box marginTop={1} flexDirection="column">
          {envs.map((env, i) => {
            const isSelected = i === selectedIdx
            return (
              <box key={env.environment_id} flexDirection="row">
                <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                  <strong>{` ${env.name} `}</strong>
                </text>
                <text fg={c.dim}> ({env.environment_id})</text>
              </box>
            )
          })}
        </box>
        <box marginTop={1}>
          <text fg={c.dim}>\u2191/\u2193 to select · Enter to confirm · Esc to cancel</text>
        </box>
      </box>
    )
  })()

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
    >
      <strong>
        <text fg={c.accent}>{DIALOG_TITLE}</text>
      </strong>
      <box marginTop={1} flexDirection="column">
        {body}
      </box>
    </box>
  )
}
