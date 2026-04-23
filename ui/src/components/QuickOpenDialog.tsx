import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/QuickOpenDialog.tsx`.
 *
 * Upstream depends on `FuzzyPicker` + `generateFileSuggestions` +
 * `readFileInRange` out of the Ink stack. The Lite frontend delegates
 * search + preview to the caller through the `search` / `previewFor`
 * async handlers. The dialog takes care of input, keyboard navigation,
 * and routing the final `open` / `insert` action — matching the
 * upstream shortcuts (Enter = open, Tab = mention, Shift+Tab = insert).
 */

type Result = string

type Props = {
  onDone: () => void
  onInsert: (text: string) => void
  /** Returns a ranked list of paths for the given query. Called on every
   *  keystroke (debounced by the upstream indexer in production). */
  search: (query: string) => Promise<Result[]>
  /** Returns the first N lines of a file for the preview pane. */
  previewFor: (path: string) => Promise<string>
  /** Called when the user confirms Enter on a result. Lite forwards to
   *  an external editor — upstream opens the file in $EDITOR. */
  onOpen?: (path: string) => void
  /** Total rows visible in the results pane. */
  visibleResults?: number
  /** Lines of preview text shown next to/below the results. */
  previewLines?: number
}

const DEFAULT_VISIBLE = 8
const DEFAULT_PREVIEW_LINES = 20

export function QuickOpenDialog({
  onDone,
  onInsert,
  onOpen,
  search,
  previewFor,
  visibleResults = DEFAULT_VISIBLE,
  previewLines = DEFAULT_PREVIEW_LINES,
}: Props) {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<Result[]>([])
  const [selected, setSelected] = useState(0)
  const [preview, setPreview] = useState<{ path: string; content: string } | null>(
    null,
  )
  const generationRef = useRef(0)

  useEffect(() => {
    const gen = ++generationRef.current
    if (!query.trim()) {
      setResults([])
      setSelected(0)
      return
    }
    let cancelled = false
    void search(query).then(items => {
      if (cancelled || gen !== generationRef.current) return
      setResults(items)
      setSelected(0)
    })
    return () => {
      cancelled = true
    }
  }, [query, search])

  useEffect(() => {
    const focused = results[selected]
    if (!focused) {
      setPreview(null)
      return
    }
    let cancelled = false
    void previewFor(focused)
      .then(content => {
        if (!cancelled) setPreview({ path: focused, content })
      })
      .catch(() => {
        if (!cancelled) setPreview({ path: focused, content: '(preview unavailable)' })
      })
    return () => {
      cancelled = true
    }
  }, [results, selected, previewFor])

  const handleOpen = useCallback(
    (path: string) => {
      onOpen?.(path)
      onDone()
    },
    [onOpen, onDone],
  )

  const handleInsert = useCallback(
    (path: string, mention: boolean) => {
      onInsert(mention ? `@${path} ` : `${path} `)
      onDone()
    },
    [onInsert, onDone],
  )

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence
    const lower = (seq?.length === 1 ? seq : name ?? '').toLowerCase()

    if (name === 'escape') {
      onDone()
      return
    }
    if (name === 'up' || (lower === 'p' && event.ctrl)) {
      setSelected(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || (lower === 'n' && event.ctrl)) {
      setSelected(idx => Math.min(Math.max(results.length - 1, 0), idx + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const r = results[selected]
      if (r) handleOpen(r)
      return
    }
    if (name === 'tab') {
      const r = results[selected]
      if (r) handleInsert(r, !event.shift)
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setQuery(q => q.slice(0, -1))
      return
    }
    if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
      setQuery(q => q + seq)
    }
  })

  const slice = results.slice(0, visibleResults)
  const previewSlice = preview?.content.split('\n').slice(0, previewLines) ?? []
  const emptyMessage = query ? 'No matching files' : 'Start typing to search…'

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
        <text fg={c.accent}>Quick Open</text>
      </strong>
      <box flexDirection="row" marginTop={1}>
        <text fg={c.dim}>Search: </text>
        <text>{query}</text>
        <text fg={c.accent}>{'\u2588'}</text>
      </box>

      <box flexDirection="column" marginTop={1} paddingLeft={1}>
        {slice.length === 0 ? (
          <text fg={c.dim}>{emptyMessage}</text>
        ) : (
          slice.map((p, i) => {
            const isSelected = i === selected
            return (
              <box key={p} flexDirection="row">
                <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.info : undefined}>
                  {isSelected ? '\u276F ' : '  '}
                  {p}
                </text>
              </box>
            )
          })
        )}
      </box>

      {preview && (
        <box flexDirection="column" marginTop={1} paddingLeft={1}>
          <text fg={c.dim}>Preview: {preview.path}</text>
          {previewSlice.map((line, i) => (
            <text key={i} fg={c.text}>{line}</text>
          ))}
        </box>
      )}

      <box marginTop={1}>
        <text fg={c.dim}>
          Enter to open · Tab to mention (@path) · Shift+Tab to insert · Esc to
          cancel
        </text>
      </box>
    </box>
  )
}
