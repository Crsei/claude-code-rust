import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../ipc/context.js'
import type { BackendMessage, FileSearchMatch } from '../ipc/protocol.js'
import { useAppState } from '../store/app-store.js'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/GlobalSearchDialog.tsx`.
 *
 * Upstream spawns `ripGrepStream` directly and streams matches into a
 * `FuzzyPicker`. cc-rust's frontend has no FS access, so the dialog
 * sends a `search_files` request through the IPC and renders the
 * `file_search_result` payload the backend handler (`ipc/file_search.rs`)
 * ships back. The lookup is debounced client-side (100ms) so typing
 * fast doesn't flood the backend.
 *
 * The dialog:
 *  1. Accepts free text in the filter box.
 *  2. Shows the match count + a "+N" marker when the backend caps the
 *     result set.
 *  3. Lets the user pick a match with Enter (`onOpen`) or insert a
 *     reference to it with Tab (`@file#Lline`) / Shift+Tab
 *     (`file:line`) — same two insertion shapes upstream supports.
 */

type Props = {
  onDone: () => void
  onInsert: (text: string) => void
}

const DEBOUNCE_MS = 100
const VISIBLE_RESULTS = 10

function generateRequestId(): string {
  return `gs-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e6).toString(36)}`
}

export function GlobalSearchDialog({ onDone, onInsert }: Props) {
  const backend = useBackend()
  const cwd = useAppState().cwd

  const [query, setQuery] = useState('')
  const [focus, setFocus] = useState(0)
  const [matches, setMatches] = useState<FileSearchMatch[]>([])
  const [truncated, setTruncated] = useState(false)
  const [searching, setSearching] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const requestIdRef = useRef<string | null>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Listen for backend replies. We use a ref to keep the handler stable
  // while tracking the most recent request id — avoids re-registering
  // the listener on every keystroke.
  useEffect(() => {
    const handler = (msg: BackendMessage) => {
      if (msg.type !== 'file_search_result') return
      if (msg.request_id !== requestIdRef.current) return
      setMatches(msg.matches)
      setTruncated(msg.truncated)
      setError(msg.error ?? null)
      setSearching(false)
    }
    backend.on('message', handler)
    return () => {
      backend.removeListener('message', handler)
    }
  }, [backend])

  const sendSearch = useCallback(
    (raw: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
      const trimmed = raw.trim()
      if (!trimmed) {
        // Reset the state so the UI shows the help line instead of
        // stale results.
        setMatches([])
        setTruncated(false)
        setError(null)
        setSearching(false)
        requestIdRef.current = null
        return
      }
      debounceRef.current = setTimeout(() => {
        const requestId = generateRequestId()
        requestIdRef.current = requestId
        setSearching(true)
        setError(null)
        backend.send({
          type: 'search_files',
          request_id: requestId,
          pattern: trimmed,
          cwd: cwd || undefined,
          case_insensitive: true,
          max_results: 500,
        })
      }, DEBOUNCE_MS)
    },
    [backend, cwd],
  )

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [])

  const handleOpen = useCallback(
    (match: FileSearchMatch) => {
      // We don't have a "open in editor" IPC yet — fall back to
      // inserting the reference, which the user can then chord into
      // `@file` mentions once we land that flow. Keeps the Enter action
      // usable rather than silently no-op.
      onInsert(`${match.file}:${match.line} `)
      onDone()
    },
    [onDone, onInsert],
  )

  const handleInsert = useCallback(
    (match: FileSearchMatch, asMention: boolean) => {
      onInsert(
        asMention
          ? `@${match.file}#L${match.line} `
          : `${match.file}:${match.line} `,
      )
      onDone()
    },
    [onDone, onInsert],
  )

  const safeIndex = Math.max(0, Math.min(focus, matches.length - 1))

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined

    if (name === 'escape') {
      onDone()
      return
    }
    if (name === 'return' || name === 'enter') {
      const active = matches[safeIndex]
      if (active) handleOpen(active)
      return
    }
    if (name === 'tab') {
      const active = matches[safeIndex]
      if (active) handleInsert(active, true)
      return
    }
    if (name === 'backtab') {
      const active = matches[safeIndex]
      if (active) handleInsert(active, false)
      return
    }
    if (name === 'up') {
      setFocus(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down') {
      setFocus(prev => Math.min(matches.length - 1, prev + 1))
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setQuery(current => {
        const next = current.slice(0, Math.max(0, current.length - 1))
        sendSearch(next)
        return next
      })
      return
    }
    if (seq && seq.length === 1 && seq.charCodeAt(0) >= 0x20) {
      setQuery(current => {
        const next = current + seq
        sendSearch(next)
        return next
      })
    }
  })

  // Reset focus when the result set shrinks past the current focus.
  useEffect(() => {
    if (safeIndex !== focus) setFocus(safeIndex)
  }, [safeIndex, focus])

  const matchLabel = useMemo(() => {
    if (error) return error
    if (matches.length === 0)
      return searching ? 'Searching…' : query ? 'No matches' : 'Type to search…'
    return `${matches.length}${truncated ? '+' : ''} matches${searching ? '…' : ''}`
  }, [error, matches.length, query, searching, truncated])

  const visible = matches.slice(0, VISIBLE_RESULTS)
  const active = matches[safeIndex]

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      title="Global Search"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="row" gap={1}>
        <text fg={c.info}>›</text>
        <text selectable>
          {query || <span fg={c.dim}>Type to search…</span>}
        </text>
      </box>
      <box marginTop={1} flexDirection="row">
        <text fg={error ? c.error : c.dim}>{matchLabel}</text>
      </box>
      <box marginTop={1} flexDirection="column">
        {visible.map((match, i) => {
          const isFocused = i === safeIndex
          return (
            <box key={`${match.file}:${match.line}:${i}`} flexDirection="row" gap={1}>
              <text fg={isFocused ? c.accent : c.dim}>
                {isFocused ? '›' : ' '}
              </text>
              <text fg={c.dim} selectable>
                {match.file}:{match.line}
              </text>
              <text fg={isFocused ? c.textBright : c.text} selectable>
                {' '}
                {match.text.trimStart()}
              </text>
            </box>
          )
        })}
      </box>
      {active && (
        <box
          marginTop={1}
          flexDirection="column"
          borderStyle="single"
          borderColor={c.dim}
          paddingX={1}
        >
          <text fg={c.dim} selectable>
            {active.file}:{active.line}
          </text>
          <text selectable>{active.text}</text>
        </box>
      )}
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Up/Down · Enter to insert path:line · Tab: @file#Lline ·
              Shift+Tab: file:line · Esc to close
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}
