import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `LogSelector`
 * (`ui/examples/upstream-patterns/src/components/LogSelector.tsx`).
 *
 * Upstream is a 1.4K-line screen that powers `claude --resume`:
 * lists prior sessions, supports fuse.js search + agentic "search with
 * Claude", branch / worktree / project-scope filtering, fork grouping,
 * inline rename, and a preview pane that reuses `SessionPreview`. Most
 * of that machinery relies on Node-side helpers
 * (`utils/getWorktreePaths`, `utils/sessionStorage`, analytics) and
 * IPC commands the Lite backend does not expose today (list sessions,
 * rename, agentic search).
 *
 * This re-host keeps upstream's `onSelect` / `onCancel` / `logs`
 * contract but narrows the UI to what the current backend supports: a
 * filterable list with search, keyboard navigation, and a selection
 * callback. Callers receive the full `LogOption` they handed in so they
 * can dispatch whatever resume command fits their IPC surface.
 *
 * Fields like `onAgenticSearch`, `onToggleAllProjects`, branch filter,
 * inline rename, and the fork tree stay on the prop surface (typed as
 * optional) so upstream callers compile — they're just inert until
 * the corresponding backend commands land.
 */

export type LogOption = {
  /** Unique identifier — typically the session id. */
  id: string
  /** Human-readable summary shown in the list. */
  summary: string
  /** Optional dimmed metadata shown on a second line (timestamp, token
   *  counts, etc.). */
  metadata?: string
  /** Optional project-scope label for multi-project views. */
  projectPath?: string
  /** Optional marker for sidechain sessions (mirrors upstream). */
  isSidechain?: boolean
  /** Optional user tags. */
  tags?: string[]
}

export type LogSelectorProps = {
  logs: LogOption[]
  maxHeight?: number
  forceWidth?: number
  onCancel?: () => void
  onSelect: (log: LogOption) => void
  onLogsChanged?: () => void
  onLoadMore?: (count: number) => void
  initialSearchQuery?: string
  showAllProjects?: boolean
  onToggleAllProjects?: () => void
  onAgenticSearch?: (
    query: string,
    logs: LogOption[],
    signal?: AbortSignal,
  ) => Promise<LogOption[]>
}

const VISIBLE_ROWS_DEFAULT = 12

function matchesQuery(log: LogOption, query: string): boolean {
  if (!query) return true
  const q = query.toLowerCase()
  if (log.summary.toLowerCase().includes(q)) return true
  if (log.metadata?.toLowerCase().includes(q)) return true
  if (log.projectPath?.toLowerCase().includes(q)) return true
  if (log.tags?.some(t => t.toLowerCase().includes(q))) return true
  return false
}

export function LogSelector({
  logs,
  maxHeight = VISIBLE_ROWS_DEFAULT,
  onCancel,
  onSelect,
  initialSearchQuery = '',
  showAllProjects,
  onToggleAllProjects,
}: LogSelectorProps) {
  const [query, setQuery] = useState(initialSearchQuery)
  const [cursor, setCursor] = useState(0)

  const filtered = useMemo(
    () => logs.filter(log => matchesQuery(log, query)),
    [logs, query],
  )

  const safeCursor = Math.max(0, Math.min(cursor, Math.max(filtered.length - 1, 0)))
  const visible = Math.max(1, Math.min(maxHeight, filtered.length))
  const firstVisible = Math.max(
    0,
    Math.min(safeCursor - Math.floor(visible / 2), filtered.length - visible),
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'up' && filtered.length > 0) {
      setCursor(i => Math.max(0, i - 1))
      return
    }
    if (name === 'down' && filtered.length > 0) {
      setCursor(i => Math.min(filtered.length - 1, i + 1))
      return
    }
    if (name === 'pageup' && filtered.length > 0) {
      setCursor(i => Math.max(0, i - visible))
      return
    }
    if (name === 'pagedown' && filtered.length > 0) {
      setCursor(i => Math.min(filtered.length - 1, i + visible))
      return
    }
    if (name === 'home') {
      setCursor(0)
      return
    }
    if (name === 'end') {
      setCursor(Math.max(0, filtered.length - 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const log = filtered[safeCursor]
      if (log) onSelect(log)
      return
    }
    if (name === 'backspace') {
      setQuery(q => q.slice(0, -1))
      setCursor(0)
      return
    }
    if (event.ctrl && (name === 'p' || seq === '\x10')) {
      onToggleAllProjects?.()
      return
    }
    if (typeof seq === 'string' && seq.length === 1 && seq >= ' ' && seq !== '\x7f') {
      setQuery(q => q + seq)
      setCursor(0)
    }
  })

  const rows = filtered.slice(firstVisible, firstVisible + visible)

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
      title="Resume session"
      titleAlignment="center"
    >
      <box flexDirection="row">
        <text fg={c.dim}>Search: </text>
        <text fg={c.text}>{query || <em><span fg={c.dim}>type to filter</span></em>}</text>
      </box>
      {onToggleAllProjects && (
        <box marginTop={1}>
          <text fg={c.dim}>
            <em>
              Ctrl+P to {showAllProjects ? 'hide' : 'show'} all projects
            </em>
          </text>
        </box>
      )}

      {filtered.length === 0 ? (
        <box marginTop={1}>
          <text fg={c.dim}>No matching sessions.</text>
        </box>
      ) : (
        <box flexDirection="column" marginTop={1}>
          {rows.map((log, i) => {
            const idx = firstVisible + i
            const isFocused = idx === safeCursor
            return (
              <box key={log.id} flexDirection="column" height={log.metadata ? 2 : 1}>
                <text>
                  <span fg={isFocused ? c.accent : c.dim}>
                    {isFocused ? '\u25B8 ' : '  '}
                  </span>
                  <span fg={isFocused ? c.textBright : c.text}>{log.summary}</span>
                  {log.isSidechain && (
                    <span fg={c.dim}> (sidechain)</span>
                  )}
                </text>
                {log.metadata && (
                  <text>
                    <span fg={c.dim}>{'    '}{log.metadata}</span>
                    {log.projectPath && (
                      <span fg={c.dim}>{` · ${log.projectPath}`}</span>
                    )}
                  </text>
                )}
              </box>
            )
          })}
        </box>
      )}

      <box marginTop={1}>
        <text fg={c.dim}>
          <em>
            {filtered.length > 0
              ? `${safeCursor + 1} / ${filtered.length} · Enter open · Esc close`
              : 'Esc close'}
          </em>
        </text>
      </box>
    </box>
  )
}
