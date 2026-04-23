import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/memory/MemoryFileSelector.tsx`.
 *
 * Upstream talks directly to `getMemoryFiles()`, `isAutoMemoryEnabled()`,
 * `getAutoMemPath()`, and the TEAMMEM + AutoDream helpers — all of which
 * read disk / remote-config state from the Ink runtime. cc-rust has no
 * matching frontend helpers; the equivalent data lives behind the
 * `/memory` backend command. We keep the upstream component shape by
 * taking the list of memory entries as a prop + three callbacks so the
 * host can plug in whichever source it has:
 *
 * 1. Parents populate `entries` from either the backend's `/memory`
 *    snapshot or a locally-walked CLAUDE.md list.
 * 2. `onSelect(path)` fires when the user accepts an entry — the
 *    parent is responsible for opening the editor / dispatching the
 *    backend command.
 * 3. `onOpenFolder(path)` fires for the "Open auto-memory folder"
 *    options and `onToggleAutoMemory` / `onToggleAutoDream` let the
 *    host flip the user-settings flags via whatever channel it uses.
 *
 * The selector mirrors upstream's visual layout — toggle rows above,
 * memory list below, "L " indent for nested `@`-imports — without
 * trying to reimplement upstream's disk walking inside the frontend.
 */

export interface MemoryEntry {
  /** Absolute path for file entries. For folder actions use `folder:<path>`. */
  value: string
  /** Human-readable label. */
  label: string
  /** Secondary line — "Saved in ~/.claude/CLAUDE.md" etc. */
  description?: string
  /** `true` when the backend reports the file does not yet exist —
   *  appended as " (new)" next to the label. */
  isNew?: boolean
  /** Indent depth for nested `@`-imports. 0 = top level. */
  depth?: number
  /** Kind discriminant used for styling. Folder entries render the
   *  "Open …" label without the memory gutter. */
  kind: 'memory' | 'folder'
}

type Props = {
  entries: MemoryEntry[]
  onSelect: (entry: MemoryEntry) => void
  onCancel: () => void
  /** Show the "Auto-memory: on/off" row. */
  autoMemoryOn?: boolean
  /** Toggle callback — parent persists the user setting. */
  onToggleAutoMemory?: () => void
  /** Show the "Auto-dream: on/off" row (defaults to off). */
  autoDreamOn?: boolean
  onToggleAutoDream?: () => void
  /** When auto-dream is configured, an optional status line — "last
   *  ran 3m ago" / "running" / "never". */
  dreamStatus?: string
  /** Pre-select an entry by `value`. */
  defaultValue?: string
}

type Focus = 'toggle-auto-memory' | 'toggle-auto-dream' | 'list'

export function MemoryFileSelector({
  entries,
  onSelect,
  onCancel,
  autoMemoryOn = false,
  onToggleAutoMemory,
  autoDreamOn = false,
  onToggleAutoDream,
  dreamStatus,
  defaultValue,
}: Props) {
  const showDreamRow = autoDreamOn || Boolean(dreamStatus)
  const togglesAvailable = useMemo<Focus[]>(() => {
    const toggles: Focus[] = []
    if (onToggleAutoMemory) toggles.push('toggle-auto-memory')
    if (showDreamRow) toggles.push('toggle-auto-dream')
    return toggles
  }, [onToggleAutoMemory, showDreamRow])

  const initialListIndex = useMemo(() => {
    if (!defaultValue) return 0
    const idx = entries.findIndex(e => e.value === defaultValue)
    return idx >= 0 ? idx : 0
  }, [entries, defaultValue])

  const [focus, setFocus] = useState<Focus>('list')
  const [listIndex, setListIndex] = useState(initialListIndex)
  const safeListIndex = entries.length === 0 ? -1 : Math.max(0, Math.min(listIndex, entries.length - 1))

  const activateFocusUp = () => {
    if (focus === 'list') {
      if (togglesAvailable.length > 0) {
        setFocus(togglesAvailable[togglesAvailable.length - 1]!)
      }
      return
    }
    const pos = togglesAvailable.indexOf(focus)
    if (pos > 0) setFocus(togglesAvailable[pos - 1]!)
  }

  const activateFocusDown = () => {
    if (focus === 'list') {
      setListIndex(prev => Math.min(entries.length - 1, prev + 1))
      return
    }
    const pos = togglesAvailable.indexOf(focus)
    if (pos >= 0 && pos < togglesAvailable.length - 1) {
      setFocus(togglesAvailable[pos + 1]!)
    } else {
      setFocus('list')
    }
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'up' || input === 'k') {
      if (focus === 'list') {
        if (safeListIndex > 0) {
          setListIndex(prev => Math.max(0, prev - 1))
          return
        }
        activateFocusUp()
        return
      }
      activateFocusUp()
      return
    }
    if (name === 'down' || input === 'j') {
      activateFocusDown()
      return
    }
    if (name === 'return' || name === 'enter') {
      if (focus === 'toggle-auto-memory') {
        onToggleAutoMemory?.()
        return
      }
      if (focus === 'toggle-auto-dream') {
        onToggleAutoDream?.()
        return
      }
      const entry = entries[safeListIndex]
      if (entry) onSelect(entry)
    }
  })

  return (
    <box flexDirection="column" width="100%">
      <box flexDirection="column" marginBottom={1}>
        {onToggleAutoMemory && (
          <ToggleRow
            focused={focus === 'toggle-auto-memory'}
            label={`Auto-memory: ${autoMemoryOn ? 'on' : 'off'}`}
          />
        )}
        {showDreamRow && (
          <ToggleRow
            focused={focus === 'toggle-auto-dream'}
            label={`Auto-dream: ${autoDreamOn ? 'on' : 'off'}${dreamStatus ? ` · ${dreamStatus}` : ''}`}
          />
        )}
      </box>

      {entries.length === 0 ? (
        <text fg={c.dim}>
          <em>(no memory files discovered)</em>
        </text>
      ) : (
        <box flexDirection="column">
          {entries.map((entry, i) => {
            const isFocused = focus === 'list' && i === safeListIndex
            const indent = entry.depth && entry.depth > 0 ? '  '.repeat(entry.depth - 1) : ''
            const prefix = entry.kind === 'memory' && entry.depth && entry.depth > 0 ? `${indent}L ` : ''
            const newSuffix = entry.isNew ? ' (new)' : ''
            return (
              <box key={entry.value} flexDirection="row" width="100%">
                <text fg={isFocused ? c.accent : c.dim}>
                  {isFocused ? '\u203A' : ' '}
                </text>
                <box flexDirection="column" paddingLeft={1}>
                  <text
                    fg={isFocused ? c.textBright : c.text}
                    selectable
                  >
                    {prefix}
                    {entry.label}
                    {newSuffix}
                  </text>
                  {entry.description && (
                    <text fg={c.dim} selectable>
                      {entry.description}
                    </text>
                  )}
                </box>
              </box>
            )
          })}
        </box>
      )}

      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Up/Down to move · Enter to select · Esc to cancel
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}

function ToggleRow({ focused, label }: { focused: boolean; label: string }) {
  return (
    <box flexDirection="row">
      <text fg={focused ? c.accent : c.dim}>
        {focused ? '\u203A' : ' '}
      </text>
      <text fg={focused ? c.textBright : c.text} paddingLeft={1}>
        {label}
      </text>
    </box>
  )
}
