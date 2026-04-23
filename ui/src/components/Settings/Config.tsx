import React, { useCallback, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useAppDispatch, useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/Settings/Config.tsx`.
 *
 * Upstream's `Config.tsx` is a 2 000+-line orchestrator that surfaces
 * every tunable in `settings.json` alongside ~20 pickers
 * (ThemePicker, ModelPicker, OutputStylePicker, LanguagePicker,
 * ChannelDowngradeDialog, …). Almost every picker depends on
 * upstream-only concepts that cc-rust does not model (fast mode,
 * auto-mode, teammate mode, bridge/channel tooling, billed-as-extra-
 * usage). Porting it verbatim would require dozens of new IPC
 * commands.
 *
 * This Lite version keeps the *shape* of upstream:
 *
 * - A header row + searchable option list.
 * - Options grouped into "Core" / "Editor" / "View" / "Managed".
 * - Enter toggles the highlighted option where the store-backed
 *   state supports it (vim toggle + view mode toggle today); other
 *   rows render a read-only description that parents can upgrade
 *   without touching the renderer.
 *
 * The component takes `extraOptions` so hosts can feed in sandbox,
 * MCP toggles, agent-settings entry points, etc. — keeping this file
 * small until the matching backend IPC lands.
 */

export interface ConfigOption {
  /** Stable identifier — used for keyboard navigation + search. */
  id: string
  label: string
  description: string
  /** Rendered next to the label ("on" / "off" / a model id). */
  value: string
  /** When set, Enter calls this callback; otherwise the row is
   *  read-only and enter is a no-op. */
  onActivate?: () => void | Promise<void>
  /** Marks the option as "managed" so it renders in gray and the
   *  activate handler is ignored. */
  managed?: boolean
  /** Optional category label used for the section headings. */
  section?: 'Core' | 'Editor' | 'View' | 'Managed' | string
}

type Props = {
  /** Host-supplied option list appended after the built-in rows. */
  extraOptions?: ConfigOption[]
  /** Width override — used inside the settings modal to shrink the
   *  list. */
  contentHeight?: number
  onClose: (result?: string) => void
}

export function Config({ extraOptions = [], contentHeight, onClose }: Props) {
  const state = useAppState()
  const dispatch = useAppDispatch()
  const [query, setQuery] = useState('')
  const [cursor, setCursor] = useState(0)

  const builtins: ConfigOption[] = useMemo(() => {
    return [
      {
        id: 'vim',
        label: 'Vim mode',
        description: 'Toggle vim-style keybindings for the composer',
        value: state.vimEnabled ? 'on' : 'off',
        section: 'Editor',
        onActivate: () => dispatch({ type: 'TOGGLE_VIM' }),
      },
      {
        id: 'view-mode',
        label: 'View mode',
        description:
          'Switch between the live prompt pane and the scrollable transcript view',
        value: state.viewMode,
        section: 'View',
        onActivate: () => dispatch({ type: 'TOGGLE_VIEW_MODE' }),
      },
      {
        id: 'editor-mode',
        label: 'Editor mode',
        description: 'Composer editor mode — reported by the backend',
        value: state.editorMode,
        section: 'Editor',
      },
      {
        id: 'model',
        label: 'Model',
        description:
          'Active model for the main conversation loop — change via /model',
        value: state.model || '—',
        section: 'Core',
      },
      {
        id: 'session-id',
        label: 'Session ID',
        description: 'Unique identifier for the current conversation',
        value: state.sessionId || '—',
        section: 'Core',
        managed: true,
      },
      {
        id: 'cwd',
        label: 'Working directory',
        description: 'Filesystem root used by file-scoped tools',
        value: state.cwd || '—',
        section: 'Core',
        managed: true,
      },
      {
        id: 'ide',
        label: 'IDE integration',
        description:
          'Connection state for the active IDE extension — toggle via /ide',
        value: state.ide.connected ? 'connected' : 'not connected',
        section: 'View',
      },
    ]
  }, [
    dispatch,
    state.cwd,
    state.editorMode,
    state.ide.connected,
    state.model,
    state.sessionId,
    state.viewMode,
    state.vimEnabled,
  ])

  const allOptions = useMemo(() => [...builtins, ...extraOptions], [builtins, extraOptions])

  const filtered = useMemo(() => {
    const trimmed = query.trim().toLowerCase()
    if (!trimmed) return allOptions
    return allOptions.filter(opt =>
      `${opt.label} ${opt.description} ${opt.value}`
        .toLowerCase()
        .includes(trimmed),
    )
  }, [allOptions, query])

  const safeCursor = filtered.length === 0 ? -1 : Math.max(0, Math.min(cursor, filtered.length - 1))
  const active = safeCursor >= 0 ? filtered[safeCursor] : undefined

  const activateActive = useCallback(() => {
    if (!active) return
    if (active.managed) return
    void active.onActivate?.()
  }, [active])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined

    if (name === 'escape') {
      if (query) {
        setQuery('')
        return
      }
      onClose('Config dismissed')
      return
    }
    if (name === 'up') {
      setCursor(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down') {
      setCursor(prev => Math.min(filtered.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      activateActive()
      return
    }
    if (name === 'backspace' || name === 'delete') {
      setQuery(prev => prev.slice(0, Math.max(0, prev.length - 1)))
      setCursor(0)
      return
    }
    if (seq && seq.length === 1 && seq.charCodeAt(0) >= 0x20) {
      setQuery(prev => prev + seq)
      setCursor(0)
    }
  })

  const height = contentHeight && contentHeight > 6 ? contentHeight - 4 : 12
  const visible = filtered.slice(0, Math.max(1, height))

  return (
    <box flexDirection="column" paddingY={1} gap={1} width="100%">
      <box flexDirection="row" gap={1}>
        <text fg={c.info}>\u25B8</text>
        <text selectable>
          {query || <span fg={c.dim}>Search config…</span>}
        </text>
      </box>

      {filtered.length === 0 ? (
        <text fg={c.dim}>
          <em>No options match "{query}"</em>
        </text>
      ) : (
        <box flexDirection="column">
          {visible.map((opt, i) => {
            const isFocused = i === safeCursor
            return (
              <box key={opt.id} flexDirection="row" width="100%">
                <text fg={isFocused ? c.accent : c.dim}>
                  {isFocused ? '\u203A' : ' '}
                </text>
                <box flexDirection="column" paddingLeft={1} width="100%">
                  <box flexDirection="row" gap={1}>
                    <text fg={isFocused ? c.textBright : c.text} selectable>
                      <strong>{opt.label}</strong>
                    </text>
                    <text fg={opt.managed ? c.dim : c.info} selectable>
                      {opt.value}
                    </text>
                    {opt.managed && <text fg={c.dim}>(managed)</text>}
                  </box>
                  <text fg={c.dim} selectable>
                    {opt.description}
                  </text>
                </box>
              </box>
            )
          })}
          {filtered.length > visible.length && (
            <text fg={c.dim}>
              {'\u2026 +'}
              {filtered.length - visible.length} more (keep scrolling)
            </text>
          )}
        </box>
      )}

      {active && !active.managed && active.onActivate && (
        <text fg={c.dim}>
          <em>Press Enter to toggle "{active.label}"</em>
        </text>
      )}

      <text fg={c.dim}>
        <em>Type to filter · Up/Down to move · Enter to activate · Esc to close</em>
      </text>
    </box>
  )
}
