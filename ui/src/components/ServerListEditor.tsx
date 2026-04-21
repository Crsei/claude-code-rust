import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Generic "server list + actions + edit slot" component shared by
 * `/mcp`, `/plugin`, and `/ide`. See `docs/superpowers/specs/` for the
 * rationale behind a single widget over three hand-rolled panels.
 *
 * The component is intentionally dumb: it owns cursor movement and the
 * edit-slot visibility, and delegates every side-effect to callers via
 * the `actions` + `editPanel` props. That keeps the three slash-commands
 * free to layer different persistence / IPC semantics on top.
 */

export interface ServerListColumn<T> {
  /** Optional header label (rendered dim above the column). */
  header?: string
  /** Fixed width in cells, or `'auto'` to grow. Defaults to `'auto'`. */
  width?: number | 'auto'
  /** Cell renderer. */
  render: (item: T) => React.ReactNode
}

export interface ServerListAction<T> {
  /** Lowercase single-char shortcut (`'e'`, `'x'`, …). */
  key: string
  /** Label shown in the footer. */
  label: string
  /** Whether the action applies to the currently-selected item. */
  enabled?: (item: T) => boolean
  /** Invoked when the key is pressed against a selected enabled item. */
  onSelect: (item: T) => void
}

export interface ServerListEditorProps<T> {
  /** Panel title (shown in the border). */
  title: string
  /** List of items to render. */
  items: T[]
  /** Stable key extractor — used for React keys and selection state. */
  getId: (item: T) => string
  /** Column renderers — each row calls these left-to-right. */
  columns: ServerListColumn<T>[]
  /** Row actions; the shortcut key fires `onSelect(item)` for the selected row. */
  actions?: ServerListAction<T>[]
  /** Message shown when `items` is empty. */
  emptyMessage?: string
  /** Extra footer content rendered below the action hints. */
  footer?: React.ReactNode
  /** Controlled selected id. When undefined, selection is internal. */
  selectedId?: string
  /** Called whenever the selection cursor moves. */
  onSelectionChange?: (id: string) => void
  /**
   * Optional edit-slot renderer. When supplied, pressing `'e'` (and any
   * `'edit'`-labeled action) flips the panel to the edit form; calling
   * the provided `close` callback returns to the list view.
   */
  editPanel?: (item: T, close: () => void) => React.ReactNode
}

export function ServerListEditor<T>(props: ServerListEditorProps<T>): React.ReactElement {
  const {
    title,
    items,
    getId,
    columns,
    actions = [],
    emptyMessage = 'No entries.',
    footer,
    selectedId,
    onSelectionChange,
    editPanel,
  } = props

  const [cursor, setCursor] = useState(0)
  const [editing, setEditing] = useState(false)

  // Reconcile cursor with controlled selectedId when it changes externally.
  useEffect(() => {
    if (selectedId === undefined) return
    const idx = items.findIndex(item => getId(item) === selectedId)
    if (idx >= 0) setCursor(idx)
  }, [selectedId, items, getId])

  // Clamp cursor when items shrink.
  useEffect(() => {
    if (cursor >= items.length) {
      setCursor(Math.max(0, items.length - 1))
    }
  }, [items.length, cursor])

  // Auto-exit edit mode if the selected item disappears.
  useEffect(() => {
    if (editing && items.length === 0) setEditing(false)
  }, [editing, items.length])

  const selected = items[cursor]
  const selectedIdResolved = selected ? getId(selected) : undefined

  const moveCursor = useCallback(
    (delta: number) => {
      if (items.length === 0) return
      const next = (cursor + delta + items.length) % items.length
      setCursor(next)
      const id = getId(items[next])
      if (onSelectionChange) onSelectionChange(id)
    },
    [cursor, items, getId, onSelectionChange],
  )

  const handleAction = useCallback(
    (action: ServerListAction<T>) => {
      if (!selected) return
      if (action.enabled && !action.enabled(selected)) return
      action.onSelect(selected)
    },
    [selected],
  )

  useKeyboard(e => {
    if (e.eventType === 'release') return

    // Edit-mode lets its own panel consume keys; only Escape bubbles up.
    if (editing) {
      if (e.name === 'escape') setEditing(false)
      return
    }

    if (e.name === 'up' || e.sequence === 'k') {
      moveCursor(-1)
      return
    }
    if (e.name === 'down' || e.sequence === 'j') {
      moveCursor(1)
      return
    }

    // `e` toggles the edit slot for the selected row when an editPanel
    // renderer is supplied.
    if (editPanel && selected && (e.sequence === 'e' || e.name === 'e')) {
      setEditing(true)
      return
    }

    // Action shortcuts.
    const keyed = e.sequence?.length === 1 ? e.sequence : undefined
    if (!keyed) return
    const match = actions.find(a => a.key === keyed.toLowerCase())
    if (match) handleAction(match)
  })

  const columnWidths = useMemo(
    () => columns.map(col => (col.width === undefined || col.width === 'auto' ? null : col.width)),
    [columns],
  )

  if (editing && selected && editPanel) {
    return (
      <box
        flexDirection="column"
        border
        borderStyle="rounded"
        borderColor={c.warning}
        paddingX={1}
        title={`${title} — edit`}
        titleAlignment="left"
      >
        {editPanel(selected, () => setEditing(false))}
      </box>
    )
  }

  return (
    <box
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor={c.muted}
      paddingX={1}
      title={title}
      titleAlignment="left"
    >
      {items.length === 0 ? (
        <text>
          <span fg={c.dim}>{emptyMessage}</span>
        </text>
      ) : (
        <>
          {columns.some(col => col.header) && (
            <text>
              {columns.map((col, i) => (
                <span key={`h-${i}`} fg={c.dim}>
                  {formatCell(col.header ?? '', columnWidths[i])}
                </span>
              ))}
            </text>
          )}
          {items.map((item, idx) => {
            const isSelected = idx === cursor
            const id = getId(item)
            return (
              <box key={id} flexDirection="row">
                <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                  {isSelected ? ' ▸ ' : '   '}
                </text>
                {columns.map((col, i) => (
                  <box key={`${id}-${i}`} minWidth={columnWidths[i] ?? undefined} marginRight={1}>
                    <text>{col.render(item)}</text>
                  </box>
                ))}
              </box>
            )
          })}
        </>
      )}

      {(actions.length > 0 || footer !== undefined) && (
        <box marginTop={1} flexDirection="column">
          {actions.length > 0 && (
            <text>
              <span fg={c.dim}>↑/↓ navigate · </span>
              {actions.map((action, i) => {
                const disabled = selected && action.enabled && !action.enabled(selected)
                const color = disabled ? c.muted : c.info
                return (
                  <span key={action.key}>
                    <span fg={color}>{action.key}</span>
                    <span fg={c.dim}>{` ${action.label}`}</span>
                    {i < actions.length - 1 && <span fg={c.dim}> · </span>}
                  </span>
                )
              })}
              {editPanel && (
                <>
                  <span fg={c.dim}> · </span>
                  <span fg={c.info}>e</span>
                  <span fg={c.dim}> edit</span>
                </>
              )}
            </text>
          )}
          {footer !== undefined && <box marginTop={0}>{footer}</box>}
        </box>
      )}

      {selectedIdResolved === undefined && null /* keep id referenced to satisfy controlled-mode contract */}
    </box>
  )
}

/** Right-pad or truncate `text` to `width` columns. Exported for tests. */
export function formatCell(text: string, width: number | null): string {
  if (width === null) return text
  if (text.length >= width) return text.slice(0, width)
  return text + ' '.repeat(width - text.length)
}
