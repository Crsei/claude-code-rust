import React, { useCallback, useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/ResumeTask.tsx`.
 *
 * Fetches recent Claude Code sessions for the current repo, shows them
 * in a padded list, and lets the user pick one to resume. Upstream
 * reaches through `src/utils/teleport/api.ts` + `detectRepository`;
 * the Lite port lifts those out into `loader` so the component can be
 * reused regardless of how sessions are discovered.
 */

export type CodeSession = {
  id: string
  title: string
  updated_at: string
  repo?: {
    owner: { login: string }
    name: string
  } | null
}

type LoadErrorType = 'network' | 'auth' | 'api' | 'other'

type Props = {
  onSelect: (session: CodeSession) => void
  onCancel: () => void
  loader: () => Promise<CodeSession[]>
  /** Current repository hint (e.g. `owner/name`) used in the title. */
  currentRepo?: string | null
  /** Compact mode — used by the embedded `/resume` picker. */
  isEmbedded?: boolean
}

const UPDATED_STRING = 'Updated'
const COL_SEP = '  '

function formatRelative(date: Date): string {
  const diff = Date.now() - date.getTime()
  if (diff < 0) return 'now'
  const minutes = Math.floor(diff / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days}d ago`
  const months = Math.floor(days / 30)
  if (months < 12) return `${months}mo ago`
  const years = Math.floor(months / 12)
  return `${years}y ago`
}

function classifyError(message: string): LoadErrorType {
  const m = message.toLowerCase()
  if (m.includes('fetch') || m.includes('network') || m.includes('timeout')) {
    return 'network'
  }
  if (
    m.includes('auth') ||
    m.includes('token') ||
    m.includes('permission') ||
    m.includes('oauth') ||
    m.includes('/login') ||
    m.includes('403')
  ) {
    return 'auth'
  }
  if (
    m.includes('api') ||
    m.includes('rate limit') ||
    m.includes('500') ||
    m.includes('529')
  ) {
    return 'api'
  }
  return 'other'
}

function ErrorGuidance({ kind }: { kind: LoadErrorType }) {
  switch (kind) {
    case 'network':
      return <text fg={c.dim}>Check your internet connection</text>
    case 'auth':
      return (
        <box flexDirection="column">
          <text fg={c.dim}>Teleport requires a Claude account</text>
          <text fg={c.dim}>
            Run <strong>/login</strong> and select &quot;Claude account with
            subscription&quot;
          </text>
        </box>
      )
    case 'api':
      return <text fg={c.dim}>Sorry, Claude encountered an error</text>
    case 'other':
    default:
      return <text fg={c.dim}>Sorry, Claude Code encountered an error</text>
  }
}

export function ResumeTask({
  onSelect,
  onCancel,
  loader,
  currentRepo,
  isEmbedded = false,
}: Props) {
  const [sessions, setSessions] = useState<CodeSession[]>([])
  const [loading, setLoading] = useState(true)
  const [errorKind, setErrorKind] = useState<LoadErrorType | null>(null)
  const [selected, setSelected] = useState(0)
  const [retrying, setRetrying] = useState(false)

  const load = useCallback(() => {
    setLoading(true)
    setErrorKind(null)
    loader()
      .then(result => {
        const sorted = [...result].sort(
          (a, b) =>
            new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
        )
        setSessions(sorted)
        setSelected(0)
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err)
        setErrorKind(classifyError(message))
      })
      .finally(() => {
        setLoading(false)
        setRetrying(false)
      })
  }, [loader])

  useEffect(load, [load])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      onCancel()
      return
    }
    if (errorKind !== null) {
      if (event.ctrl && key === 'r') {
        setRetrying(true)
        load()
        return
      }
      if (name === 'return' || name === 'enter') {
        onCancel()
      }
      return
    }
    if (sessions.length === 0) return
    if (name === 'up' || key === 'k') {
      setSelected(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setSelected(idx => Math.min(sessions.length - 1, idx + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const target = sessions[selected]
      if (target) onSelect(target)
    }
  })

  if (loading) {
    return (
      <box flexDirection="column" padding={1}>
        <box flexDirection="row">
          <Spinner label="Loading Claude Code sessions…" />
        </box>
        <text fg={c.dim}>
          {retrying ? 'Retrying…' : 'Fetching your Claude Code sessions…'}
        </text>
      </box>
    )
  }

  if (errorKind) {
    return (
      <box flexDirection="column" padding={1}>
        <strong>
          <text fg={c.error}>Error loading Claude Code sessions</text>
        </strong>
        <box marginY={1} flexDirection="column">
          <ErrorGuidance kind={errorKind} />
        </box>
        <text fg={c.dim}>
          Press <strong>Ctrl+R</strong> to retry · Press <strong>Esc</strong> to
          cancel
        </text>
      </box>
    )
  }

  if (sessions.length === 0) {
    return (
      <box flexDirection="column" padding={1}>
        <strong>
          <text>
            No Claude Code sessions found
            {currentRepo ? ` for ${currentRepo}` : ''}
          </text>
        </strong>
        <box marginTop={1}>
          <text fg={c.dim}>
            Press <strong>Esc</strong> to cancel
          </text>
        </box>
      </box>
    )
  }

  const metadata = sessions.map(s => ({
    ...s,
    relative: formatRelative(new Date(s.updated_at)),
  }))
  const colWidth = Math.max(
    UPDATED_STRING.length,
    ...metadata.map(m => m.relative.length),
  )
  const header = `${UPDATED_STRING.padEnd(colWidth, ' ')}${COL_SEP}Session Title`

  const visibleCount = isEmbedded
    ? Math.min(5, metadata.length)
    : Math.min(20, metadata.length)
  const startIdx = Math.max(
    0,
    Math.min(selected - Math.floor(visibleCount / 2), metadata.length - visibleCount),
  )
  const slice = metadata.slice(startIdx, startIdx + visibleCount)

  return (
    <box flexDirection="column" padding={1}>
      <strong>
        <text>
          Select a session to resume
          {metadata.length > visibleCount && (
            <text fg={c.dim}>
              {' '}
              ({selected + 1} of {metadata.length})
            </text>
          )}
          {currentRepo && <text fg={c.dim}> ({currentRepo})</text>}:
        </text>
      </strong>

      <box flexDirection="column" marginTop={1}>
        <box paddingLeft={2}>
          <strong>
            <text>{header}</text>
          </strong>
        </box>
        {slice.map((s, i) => {
          const absIdx = startIdx + i
          const isSelected = absIdx === selected
          const row = `${s.relative.padEnd(colWidth, ' ')}${COL_SEP}${s.title}`
          return (
            <box key={s.id} paddingLeft={2}>
              <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                {isSelected ? '\u276F ' : '  '}
                {row}
              </text>
            </box>
          )
        })}
      </box>

      <box marginTop={1}>
        <text fg={c.dim}>\u2191/\u2193 to select · Enter to confirm · Esc to cancel</text>
      </box>
    </box>
  )
}
