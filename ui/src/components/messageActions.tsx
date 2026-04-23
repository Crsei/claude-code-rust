import React, { createContext, useCallback, useMemo, useRef, type RefObject } from 'react'
import type { RenderItem } from '../store/message-model.js'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `messageActions.tsx`
 * (`ui/examples/upstream-patterns/src/components/messageActions.tsx`).
 *
 * Upstream wires the scrollback "cursor" bar — the `enter` / `c` / `p`
 * action row you see when you arrow up from the composer onto a prior
 * message. It reaches into upstream's `NormalizableMessage` union and a
 * global keybinding registry that Lite does not have in the same shape.
 *
 * Lite drives the transcript through `RenderItem` (pipeline lives in
 * `store/message-model.ts`). This module re-hosts the upstream surface
 * against that type:
 *   - `isNavigableMessage(item)` — decides whether an item is a cursor
 *     target.
 *   - `copyTextOf(item)` — plain-text representation for "copy".
 *   - `toolPrimaryInputOf(item)` — extracts the dominant input field
 *     (path / command / pattern / url / prompt) for the "copy X" action.
 *   - `MESSAGE_ACTIONS` — table of actions + eligibility predicates.
 *   - `useMessageActions()` — React hook returning keyboard handlers
 *     the caller can feed into a custom keybinding dispatcher.
 *   - `MessageActionsBar` — the footer row rendering the active action
 *     hints.
 *
 * Lite does not yet mount this bar inside `MessageList.tsx`; the module
 * is provided as the ready-made integration point for when the
 * navigation mode lands.
 */

export const NAVIGABLE_TYPES = [
  'user_text',
  'assistant_text',
  'tool_activity',
  'tool_group',
  'tool_result_orphan',
  'system_text',
] as const
export type NavigableType = (typeof NAVIGABLE_TYPES)[number]

export type NavigableItem = RenderItem

export function isNavigableMessage(item: NavigableItem): boolean {
  if (!NAVIGABLE_TYPES.includes(item.type as NavigableType)) return false
  if (item.type === 'system_text') {
    // Empty system messages add nothing to the cursor — skip them.
    return !!item.content.trim()
  }
  if (item.type === 'user_text' || item.type === 'assistant_text') {
    return !!item.content.trim()
  }
  if (item.type === 'tool_activity') {
    return !!item.name && !!(item.inputDetail || item.inputSummary || item.output)
  }
  if (item.type === 'tool_group') {
    return item.activities.length > 0
  }
  return true
}

type PrimaryInput = { label: string; extract: (input: any) => string | undefined }

const pickString = (key: string) => (input: any): string | undefined => {
  if (!input || typeof input !== 'object') return undefined
  const value = input[key]
  return typeof value === 'string' && value.trim() ? value : undefined
}

/** Mirror of upstream's `PRIMARY_INPUT`. */
export const PRIMARY_INPUT: Record<string, PrimaryInput> = {
  Read: { label: 'path', extract: pickString('file_path') },
  Edit: { label: 'path', extract: pickString('file_path') },
  Write: { label: 'path', extract: pickString('file_path') },
  NotebookEdit: { label: 'path', extract: pickString('notebook_path') },
  Bash: { label: 'command', extract: pickString('command') },
  Grep: { label: 'pattern', extract: pickString('pattern') },
  Glob: { label: 'pattern', extract: pickString('pattern') },
  WebFetch: { label: 'url', extract: pickString('url') },
  WebSearch: { label: 'query', extract: pickString('query') },
  Task: { label: 'prompt', extract: pickString('prompt') },
  Agent: { label: 'prompt', extract: pickString('prompt') },
}

export function toolPrimaryInputOf(
  item: NavigableItem,
): { name: string; label: string; value: string } | undefined {
  if (item.type !== 'tool_activity' && item.type !== 'tool_group') return undefined
  if (item.type === 'tool_activity') {
    const spec = PRIMARY_INPUT[item.name]
    const value = spec?.extract(item.input)
    if (!spec || !value) return undefined
    return { name: item.name, label: spec.label, value }
  }
  const first = item.activities[0]
  if (!first) return undefined
  const spec = PRIMARY_INPUT[first.name]
  const value = spec?.extract(first.input)
  if (!spec || !value) return undefined
  return { name: first.name, label: spec.label, value }
}

export function copyTextOf(item: NavigableItem): string {
  switch (item.type) {
    case 'user_text':
    case 'assistant_text':
    case 'streaming':
      return item.content
    case 'system_text':
      return item.content
    case 'tool_activity':
      return item.output ?? item.inputDetail ?? item.inputSummary ?? ''
    case 'tool_group':
      return item.activities
        .map(a => a.output ?? a.inputDetail ?? a.inputSummary ?? '')
        .filter(Boolean)
        .join('\n\n')
    case 'tool_result_orphan':
      return item.output
    default:
      return ''
  }
}

export type MessageActionCaps = {
  copy: (text: string) => void
}

export type MessageActionsState = {
  id: string
  itemType: NavigableType
  expanded: boolean
  /** Dominant tool name when the cursor sits on a tool activity / group. */
  toolName?: string
}

export type MessageActionsNav = {
  enterCursor: () => void
  navigatePrev: () => void
  navigateNext: () => void
  navigateTop: () => void
  navigateBottom: () => void
  getSelected: () => NavigableItem | null
}

export const MessageActionsSelectedContext = createContext(false)
export const InVirtualListContext = createContext(false)

/** Selected-row background colour helper — matches upstream API. */
export function useSelectedMessageBg(): string | undefined {
  return undefined
}

type Action = {
  key: string
  label: string | ((state: MessageActionsState) => string)
  types: readonly NavigableType[]
  applies?: (state: MessageActionsState) => boolean
  stays?: true
  run: (item: NavigableItem, caps: MessageActionCaps) => void
}

export const MESSAGE_ACTIONS: Action[] = [
  {
    key: 'enter',
    label: state => (state.expanded ? 'collapse' : 'expand'),
    types: ['tool_activity', 'tool_group', 'system_text'],
    stays: true,
    run: () => {},
  },
  {
    key: 'c',
    label: 'copy',
    types: NAVIGABLE_TYPES,
    run: (item, caps) => caps.copy(copyTextOf(item)),
  },
  {
    key: 'p',
    label: state =>
      state.toolName && state.toolName in PRIMARY_INPUT
        ? `copy ${PRIMARY_INPUT[state.toolName]!.label}`
        : 'copy',
    types: ['tool_activity', 'tool_group'],
    applies: state => !!state.toolName && state.toolName in PRIMARY_INPUT,
    run: (item, caps) => {
      const primary = toolPrimaryInputOf(item)
      if (primary) caps.copy(primary.value)
    },
  },
]

function isApplicable(action: Action, state: MessageActionsState): boolean {
  if (!action.types.includes(state.itemType)) return false
  return !action.applies || action.applies(state)
}

/**
 * Returns a map of keyboard-handler closures keyed by shortcut action.
 * Callers register these with the app's keybinding dispatcher. The
 * hook is context-agnostic: no internal dependency on a specific
 * keybinding surface.
 */
export function useMessageActions(
  cursor: MessageActionsState | null,
  setCursor: React.Dispatch<React.SetStateAction<MessageActionsState | null>>,
  navRef: RefObject<MessageActionsNav | null>,
  caps: MessageActionCaps,
): {
  enter: () => void
  handlers: Record<string, () => void>
} {
  const cursorRef = useRef(cursor)
  cursorRef.current = cursor
  const capsRef = useRef(caps)
  capsRef.current = caps

  const handlers = useMemo(() => {
    const h: Record<string, () => void> = {
      'messageActions:prev': () => navRef.current?.navigatePrev(),
      'messageActions:next': () => navRef.current?.navigateNext(),
      'messageActions:top': () => navRef.current?.navigateTop(),
      'messageActions:bottom': () => navRef.current?.navigateBottom(),
      'messageActions:escape': () =>
        setCursor(c => (c?.expanded ? { ...c, expanded: false } : null)),
      'messageActions:ctrlc': () => setCursor(null),
    }
    const keys = new Set(MESSAGE_ACTIONS.map(a => a.key))
    for (const key of keys) {
      h[`messageActions:${key}`] = () => {
        const current = cursorRef.current
        if (!current) return
        const action = MESSAGE_ACTIONS.find(
          a => a.key === key && isApplicable(a, current),
        )
        if (!action) return
        if (action.stays) {
          setCursor(c => (c ? { ...c, expanded: !c.expanded } : null))
          return
        }
        const item = navRef.current?.getSelected()
        if (!item) return
        action.run(item, capsRef.current)
        setCursor(null)
      }
    }
    return h
  }, [navRef, setCursor])

  const enter = useCallback(() => {
    navRef.current?.enterCursor()
  }, [navRef])

  return { enter, handlers }
}

export function MessageActionsBar({
  cursor,
}: {
  cursor: MessageActionsState
}) {
  const applicable = MESSAGE_ACTIONS.filter(a => isApplicable(a, cursor))
  return (
    <box flexDirection="column" flexShrink={0} paddingY={1}>
      <box
        borderStyle="single"
        border={['top']}
        borderColor={c.dim}
      />
      <box paddingX={2} paddingY={1} flexDirection="row">
        {applicable.map((action, i) => {
          const label =
            typeof action.label === 'function' ? action.label(cursor) : action.label
          return (
            <React.Fragment key={action.key}>
              {i > 0 && <text fg={c.dim}> · </text>}
              <text>
                <strong>{action.key}</strong>
                <span fg={c.dim}> {label}</span>
              </text>
            </React.Fragment>
          )
        })}
        <text fg={c.dim}> · </text>
        <text>
          <strong>\u2191\u2193</strong>
          <span fg={c.dim}> navigate</span>
        </text>
        <text fg={c.dim}> · </text>
        <text>
          <strong>esc</strong>
          <span fg={c.dim}> back</span>
        </text>
      </box>
    </box>
  )
}

/** Upstream's XML-trim helper. Re-exported so consumers can reuse it. */
export function stripSystemReminders(text: string): string {
  const CLOSE = '</system-reminder>'
  let t = text.trimStart()
  while (t.startsWith('<system-reminder>')) {
    const end = t.indexOf(CLOSE)
    if (end < 0) break
    t = t.slice(end + CLOSE.length).trimStart()
  }
  return t
}
