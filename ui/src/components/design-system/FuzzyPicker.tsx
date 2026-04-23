import React, { useEffect, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `FuzzyPicker`. A search input on top
 * of a ranked list with an optional preview pane. Upstream offered a
 * rich API (debouncing, prefix filters, tag pills); the Lite port keeps
 * the feature set actually used by `QuickOpenDialog` and the transcript
 * history picker:
 *
 *  - `onQueryChange` runs on every keystroke; `items` is the full,
 *    caller-ranked list.
 *  - Enter → `onSelect(item)`; Tab / Shift+Tab → optional side actions;
 *    Esc → `onCancel`.
 *  - `previewPosition` toggles between bottom + right layouts.
 */

type TabAction<T> = {
  action?: string
  handler: (item: T) => void
}

type Props<T> = {
  title: string
  placeholder?: string
  items: T[]
  getKey: (item: T) => string
  visibleCount?: number
  /** Whether results grow upward (for bottom-anchored pickers) or
   *  downward (default). */
  direction?: 'up' | 'down'
  /** `bottom` stacks preview under the list; `right` splits the row. */
  previewPosition?: 'bottom' | 'right'
  selectAction?: string
  onQueryChange?: (query: string) => void
  onFocus?: (item: T) => void
  onSelect?: (item: T) => void
  onTab?: TabAction<T>
  onShiftTab?: TabAction<T>
  onCancel?: () => void
  emptyMessage?: (query: string) => string
  renderItem?: (item: T, isFocused: boolean) => React.ReactNode
  renderPreview?: (item: T) => React.ReactNode
}

export function FuzzyPicker<T>({
  title,
  placeholder = 'Type to search…',
  items,
  getKey,
  visibleCount = 8,
  direction = 'down',
  previewPosition = 'bottom',
  selectAction = 'open',
  onQueryChange,
  onFocus,
  onSelect,
  onTab,
  onShiftTab,
  onCancel,
  emptyMessage,
  renderItem,
  renderPreview,
}: Props<T>) {
  const [query, setQuery] = useState('')
  const [focused, setFocused] = useState(0)
  const focusedRef = useRef(focused)
  focusedRef.current = focused

  useEffect(() => {
    onQueryChange?.(query)
  }, [query, onQueryChange])

  useEffect(() => {
    setFocused(0)
  }, [items])

  useEffect(() => {
    const item = items[focused]
    if (item !== undefined) onFocus?.(item)
  }, [items, focused, onFocus])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const shift = event.shift

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'return' || name === 'enter') {
      const it = items[focusedRef.current]
      if (it !== undefined) onSelect?.(it)
      return
    }
    if (name === 'tab') {
      const it = items[focusedRef.current]
      if (it === undefined) return
      if (shift) onShiftTab?.handler(it)
      else onTab?.handler(it)
      return
    }
    if (name === 'up') {
      setFocused(f => Math.max(0, f - 1))
      return
    }
    if (name === 'down') {
      setFocused(f => Math.min(Math.max(items.length - 1, 0), f + 1))
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setQuery(q => q.slice(0, -1))
      return
    }
    if (seq && !event.ctrl && !event.meta) {
      setQuery(q => q + seq)
    }
  })

  const slice = items.slice(0, visibleCount)
  const focusedItem = items[focused]
  const empty = emptyMessage?.(query) ?? (query ? 'No matches' : placeholder)

  const list = (
    <box flexDirection={direction === 'up' ? 'column-reverse' : 'column'}>
      {slice.length === 0 ? (
        <text fg={c.dim}>{empty}</text>
      ) : (
        slice.map((item, i) => {
          const isFocused = i === focused
          const keyStr = getKey(item)
          return (
            <box key={keyStr} flexDirection="row">
              <text fg={isFocused ? c.bg : undefined} bg={isFocused ? c.info : undefined}>
                {isFocused ? '\u276F ' : '  '}
                {renderItem ? null : keyStr}
              </text>
              {renderItem && renderItem(item, isFocused)}
            </box>
          )
        })
      )}
    </box>
  )

  const preview =
    focusedItem && renderPreview ? (
      <box flexDirection="column" paddingLeft={1}>
        {renderPreview(focusedItem)}
      </box>
    ) : null

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
    >
      <strong>
        <text fg={c.accent}>{title}</text>
      </strong>
      <box flexDirection="row" marginTop={1}>
        <text fg={c.dim}>Search: </text>
        <text>{query}</text>
        <text fg={c.accent}>{'\u2588'}</text>
      </box>

      {previewPosition === 'right' && preview ? (
        <box flexDirection="row" marginTop={1} gap={2}>
          <box flexDirection="column" flexGrow={1}>{list}</box>
          <box flexDirection="column" flexGrow={1}>{preview}</box>
        </box>
      ) : (
        <>
          <box marginTop={1}>{list}</box>
          {preview && <box marginTop={1}>{preview}</box>}
        </>
      )}

      <box marginTop={1}>
        <text fg={c.dim}>
          Enter {selectAction}
          {onTab?.action ? ` · Tab ${onTab.action}` : ''}
          {onShiftTab?.action ? ` · Shift+Tab ${onShiftTab.action}` : ''}
          {' · Esc cancel'}
        </text>
      </box>
    </box>
  )
}
