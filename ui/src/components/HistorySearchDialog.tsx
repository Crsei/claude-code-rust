import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useAppState } from '../store/app-store.js'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/HistorySearchDialog.tsx`.
 *
 * Upstream builds a timestamped reader off `getTimestampedHistory()`
 * and hands the results to `FuzzyPicker`. cc-rust's history lives in
 * `state.inputHistory` (populated by the Rust backend when the
 * conversation starts) — no timestamps, no lazy loading. We feed that
 * array through the same substring + subsequence matcher upstream uses
 * and render a vertical select with an in-line preview pane, which
 * is the Lite version of FuzzyPicker's two-pane layout.
 *
 * Callers provide `onSelect(text)` to receive the picked entry and
 * `onCancel()` for Esc. Use `initialQuery` to pre-seed the filter (eg.
 * when the user hit Ctrl+R in the middle of typing).
 */

type Props = {
  initialQuery?: string
  onSelect: (text: string) => void
  onCancel: () => void
}

type ScoredEntry = {
  text: string
  firstLine: string
  exact: boolean
}

const VISIBLE_RESULTS = 8
const PREVIEW_ROWS = 6

function firstLineOf(text: string): string {
  const nl = text.indexOf('\n')
  return nl === -1 ? text : text.slice(0, nl)
}

function isSubsequence(text: string, query: string): boolean {
  let j = 0
  for (let i = 0; i < text.length && j < query.length; i++) {
    if (text[i] === query[j]) j++
  }
  return j === query.length
}

function filterEntries(entries: string[], query: string): ScoredEntry[] {
  const trimmed = query.trim().toLowerCase()
  if (!trimmed) {
    return entries.map(text => ({
      text,
      firstLine: firstLineOf(text),
      exact: true,
    }))
  }

  const exact: ScoredEntry[] = []
  const fuzzy: ScoredEntry[] = []
  for (const entry of entries) {
    const lower = entry.toLowerCase()
    const firstLine = firstLineOf(entry)
    if (lower.includes(trimmed)) {
      exact.push({ text: entry, firstLine, exact: true })
    } else if (isSubsequence(lower, trimmed)) {
      fuzzy.push({ text: entry, firstLine, exact: false })
    }
  }
  return exact.concat(fuzzy)
}

export function HistorySearchDialog({
  initialQuery = '',
  onSelect,
  onCancel,
}: Props) {
  const { inputHistory } = useAppState()
  const [query, setQuery] = useState(initialQuery)
  const [focus, setFocus] = useState(0)

  const filtered = useMemo(
    () => filterEntries(inputHistory, query),
    [inputHistory, query],
  )

  // Clamp focus when the filter shrinks.
  useEffect(() => {
    if (focus >= filtered.length) {
      setFocus(Math.max(0, filtered.length - 1))
    }
  }, [filtered.length, focus])

  const safeIndex = Math.max(0, Math.min(focus, filtered.length - 1))
  const activeEntry = filtered[safeIndex]

  const handleSelect = useCallback(() => {
    const entry = filtered[safeIndex]
    if (!entry) return
    onSelect(entry.text)
  }, [filtered, onSelect, safeIndex])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined

    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'return' || name === 'enter') {
      handleSelect()
      return
    }
    if (name === 'up') {
      setFocus(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down') {
      setFocus(prev => Math.min(filtered.length - 1, prev + 1))
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setQuery(current => current.slice(0, Math.max(0, current.length - 1)))
      return
    }
    if (seq && seq.length === 1 && seq.charCodeAt(0) >= 0x20) {
      setQuery(current => current + seq)
    }
  })

  const visible = filtered.slice(0, VISIBLE_RESULTS)

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      title="Search prompts"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="row" gap={1}>
        <text fg={c.info}>›</text>
        <text fg={c.text} selectable>
          {query || (
            <span fg={c.dim}>Filter history…</span>
          )}
        </text>
      </box>

      <box marginTop={1} flexDirection="column">
        {visible.length === 0 && (
          <text fg={c.dim}>
            <em>
              {inputHistory.length === 0
                ? 'No history yet'
                : query
                  ? 'No matching prompts'
                  : 'No history yet'}
            </em>
          </text>
        )}
        {visible.map((entry, i) => {
          const isFocused = i === safeIndex
          const marker = isFocused ? '›' : ' '
          return (
            <box key={`${entry.text.slice(0, 40)}:${i}`} flexDirection="row" gap={1}>
              <text fg={isFocused ? c.accent : c.dim}>{marker}</text>
              <text
                fg={isFocused ? c.textBright : c.text}
                selectable
              >
                {entry.firstLine}
              </text>
              {!entry.exact && (
                <text fg={c.dim}> (fuzzy)</text>
              )}
            </box>
          )
        })}
      </box>

      {activeEntry && (
        <box
          marginTop={1}
          flexDirection="column"
          borderStyle="single"
          borderColor={c.dim}
          paddingX={1}
        >
          {activeEntry.text
            .split('\n')
            .slice(0, PREVIEW_ROWS)
            .map((line, i) => (
              <text key={i} fg={c.dim} selectable>
                {line.length === 0 ? ' ' : line}
              </text>
            ))}
          {activeEntry.text.split('\n').length > PREVIEW_ROWS && (
            <text fg={c.dim}>
              {'\u2026 +'}
              {activeEntry.text.split('\n').length - PREVIEW_ROWS} more lines
            </text>
          )}
        </box>
      )}

      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              {filtered.length > VISIBLE_RESULTS
                ? `${filtered.length} matches (showing ${VISIBLE_RESULTS}) · `
                : `${filtered.length} matches · `}
              Up/Down · Enter to use · Esc to cancel
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}
