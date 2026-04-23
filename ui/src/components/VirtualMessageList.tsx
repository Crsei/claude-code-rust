import React, {
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/VirtualMessageList.tsx`.
 *
 * The upstream file is a 1000+ line Ink virtualiser with:
 *  - A `useVirtualScroll` hook that cooperates with Ink's renderer.
 *  - A sticky-prompt tracker driven by `ScrollChromeContext`.
 *  - Incremental search + jump handles wired through `DOMElement` /
 *    `MatchPosition` — both Ink primitives.
 *  - A `VirtualItem` wrapper that talks to Ink's measurement loop.
 *
 * OpenTUI doesn't expose the same measurement surface, so this port
 * keeps the *interface* (`messages`, `itemKey`, `renderItem`,
 * `selectedIndex`, `onItemClick`, `extractSearchText`) and does a
 * lightweight virtualisation: it renders a windowed slice around the
 * selected / latest index and lets OpenTUI's native `<scrollbox>` do
 * the scroll chrome. `setCursor` / `cursorNavRef` are still honoured so
 * keyboard navigation keeps working, but the sticky-prompt and
 * `scanElement` search integrations degrade to no-ops — the search
 * handle stays available so callers don't need conditional wiring.
 */

export type RenderableMessage = {
  type?: string
  uuid?: string
  /** Escape hatch — the Rust port carries the structured message directly
   *  on this prop, so this type stays intentionally loose. */
  [key: string]: unknown
}

export type ScrollBoxHandle = {
  scrollBy?: (amount: number) => void
  scrollTo?: (offset: number) => void
  scrollToBottom?: () => void
}

export type MatchPosition = {
  row: number
  startCol: number
  endCol: number
}

export type MessageActionsState = {
  uuid?: string
  msgType?: string
  expanded: boolean
  toolName?: string
}

export type MessageActionsNav = {
  enterCursor: () => boolean
  navigatePrev: () => boolean
  navigateNext: () => boolean
  navigatePrevUser: () => boolean
  navigateNextUser: () => boolean
  navigateTop: () => boolean
  navigateBottom: () => boolean
  getSelected: () => RenderableMessage | null
}

export type JumpHandle = {
  jumpToIndex: (i: number) => void
  setSearchQuery: (q: string) => void
  nextMatch: () => void
  prevMatch: () => void
  setAnchor: () => void
  warmSearchIndex: () => Promise<number>
  disarmSearch: () => void
}

type Props = {
  messages: RenderableMessage[]
  scrollRef: React.RefObject<ScrollBoxHandle | null>
  columns: number
  itemKey: (msg: RenderableMessage) => string
  renderItem: (msg: RenderableMessage, index: number) => React.ReactNode
  onItemClick?: (msg: RenderableMessage) => void
  isItemClickable?: (msg: RenderableMessage) => boolean
  isItemExpanded?: (msg: RenderableMessage) => boolean
  extractSearchText?: (msg: RenderableMessage) => string
  trackStickyPrompt?: boolean
  selectedIndex?: number
  cursorNavRef?: React.Ref<MessageActionsNav>
  setCursor?: (c: MessageActionsState | null) => void
  jumpRef?: React.RefObject<JumpHandle | null>
  onSearchMatchesChange?: (count: number, current: number) => void
  setPositions?: (
    state: {
      positions: MatchPosition[]
      rowOffset: number
      currentIdx: number
    } | null,
  ) => void
}

/** How many messages to render around the anchor. Upstream's virtualiser
 *  is driven by measured heights; the Rust port picks a fixed window
 *  that comfortably covers a full terminal at 4 rows/message. */
const WINDOW_RADIUS = 40
const fallbackLowerCache = new WeakMap<RenderableMessage, string>()

function defaultExtractSearchText(msg: RenderableMessage): string {
  const cached = fallbackLowerCache.get(msg)
  if (cached !== undefined) return cached
  const candidate =
    typeof (msg as { text?: unknown }).text === 'string'
      ? (msg as { text: string }).text
      : ''
  const lowered = candidate.toLowerCase()
  fallbackLowerCache.set(msg, lowered)
  return lowered
}

export function VirtualMessageList({
  messages,
  scrollRef,
  columns,
  itemKey,
  renderItem,
  onItemClick,
  isItemClickable,
  isItemExpanded,
  extractSearchText = defaultExtractSearchText,
  selectedIndex,
  cursorNavRef,
  setCursor,
  jumpRef,
  onSearchMatchesChange,
  setPositions,
}: Props): React.ReactElement {
  // Preserve the key-array optimisation upstream uses — the full rebuild
  // only happens when the prefix changes, otherwise we append in place.
  const keysRef = useRef<string[]>([])
  const prevItemKeyRef = useRef(itemKey)
  const prevMessagesRef = useRef<RenderableMessage[]>(messages)
  if (
    prevItemKeyRef.current !== itemKey ||
    messages.length < keysRef.current.length ||
    messages[0] !== prevMessagesRef.current[0]
  ) {
    keysRef.current = messages.map(m => itemKey(m))
  } else {
    for (let i = keysRef.current.length; i < messages.length; i++) {
      keysRef.current.push(itemKey(messages[i]!))
    }
  }
  prevMessagesRef.current = messages
  prevItemKeyRef.current = itemKey

  const [searchMatches, setSearchMatches] = useState<number[]>([])
  const [currentMatchIdx, setCurrentMatchIdx] = useState<number>(-1)
  const [searchQuery, setSearchQuery] = useState('')
  const indexWarmedRef = useRef(false)

  const anchor = selectedIndex ?? Math.max(0, messages.length - 1)
  const windowStart = Math.max(0, anchor - WINDOW_RADIUS)
  const windowEnd = Math.min(messages.length, anchor + WINDOW_RADIUS + 1)

  const isNavigable = useCallback(
    (i: number) => {
      const msg = messages[i]
      if (!msg) return false
      const type = typeof msg.type === 'string' ? msg.type : ''
      return type === 'user' || type === 'assistant' || type === 'attachment'
    },
    [messages],
  )

  useImperativeHandle(
    cursorNavRef,
    () => {
      const select = (m: RenderableMessage) =>
        setCursor?.({
          uuid: typeof m.uuid === 'string' ? m.uuid : undefined,
          msgType: typeof m.type === 'string' ? m.type : undefined,
          expanded: false,
        })
      const selIdx = selectedIndex ?? -1
      const scan = (
        from: number,
        dir: 1 | -1,
        pred: (i: number) => boolean = isNavigable,
      ) => {
        for (let i = from; i >= 0 && i < messages.length; i += dir) {
          if (pred(i)) {
            select(messages[i]!)
            return true
          }
        }
        return false
      }
      const isUser = (i: number) =>
        isNavigable(i) && messages[i]?.type === 'user'
      return {
        enterCursor: () => scan(messages.length - 1, -1, isUser),
        navigatePrev: () => scan(selIdx - 1, -1),
        navigateNext: () => {
          if (scan(selIdx + 1, 1)) return true
          scrollRef.current?.scrollToBottom?.()
          setCursor?.(null)
          return false
        },
        navigatePrevUser: () => scan(selIdx - 1, -1, isUser),
        navigateNextUser: () => scan(selIdx + 1, 1, isUser),
        navigateTop: () => scan(0, 1),
        navigateBottom: () => scan(messages.length - 1, -1),
        getSelected: () => (selIdx >= 0 ? (messages[selIdx] ?? null) : null),
      }
    },
    [isNavigable, messages, scrollRef, selectedIndex, setCursor],
  )

  const recomputeMatches = useCallback(
    (query: string) => {
      if (!query) {
        setSearchMatches([])
        setCurrentMatchIdx(-1)
        onSearchMatchesChange?.(0, 0)
        setPositions?.(null)
        return
      }
      const needle = query.toLowerCase()
      const matches: number[] = []
      for (let i = 0; i < messages.length; i++) {
        if (extractSearchText(messages[i]!).includes(needle)) matches.push(i)
      }
      setSearchMatches(matches)
      if (matches.length === 0) {
        setCurrentMatchIdx(-1)
        onSearchMatchesChange?.(0, 0)
        setPositions?.(null)
        return
      }
      const anchoredIdx = Math.max(
        0,
        matches.findIndex(i => i >= anchor),
      )
      const next = anchoredIdx >= 0 ? anchoredIdx : 0
      setCurrentMatchIdx(next)
      onSearchMatchesChange?.(matches.length, next + 1)
    },
    [anchor, extractSearchText, messages, onSearchMatchesChange, setPositions],
  )

  useImperativeHandle(
    jumpRef,
    () => ({
      jumpToIndex: (i: number) => {
        if (i < 0 || i >= messages.length) return
        setCursor?.({
          uuid: typeof messages[i]?.uuid === 'string'
            ? (messages[i]!.uuid as string)
            : undefined,
          msgType: typeof messages[i]?.type === 'string'
            ? (messages[i]!.type as string)
            : undefined,
          expanded: false,
        })
      },
      setSearchQuery: (q: string) => {
        setSearchQuery(q)
        recomputeMatches(q)
      },
      nextMatch: () => {
        if (searchMatches.length === 0) return
        const next = (currentMatchIdx + 1) % searchMatches.length
        setCurrentMatchIdx(next)
        onSearchMatchesChange?.(searchMatches.length, next + 1)
      },
      prevMatch: () => {
        if (searchMatches.length === 0) return
        const next =
          (currentMatchIdx - 1 + searchMatches.length) % searchMatches.length
        setCurrentMatchIdx(next)
        onSearchMatchesChange?.(searchMatches.length, next + 1)
      },
      setAnchor: () => {
        // Upstream captures the current scrollTop here; OpenTUI doesn't
        // surface a read-side scroll offset so this is a no-op.
      },
      warmSearchIndex: async () => {
        if (indexWarmedRef.current) return 0
        const started = Date.now()
        for (const msg of messages) extractSearchText(msg)
        indexWarmedRef.current = true
        return Date.now() - started
      },
      disarmSearch: () => {
        setSearchMatches([])
        setCurrentMatchIdx(-1)
        onSearchMatchesChange?.(0, 0)
        setPositions?.(null)
      },
    }),
    [
      currentMatchIdx,
      extractSearchText,
      messages,
      onSearchMatchesChange,
      recomputeMatches,
      searchMatches.length,
      setCursor,
      setPositions,
    ],
  )

  useEffect(() => {
    if (searchQuery) recomputeMatches(searchQuery)
  }, [recomputeMatches, searchQuery])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const scroll = scrollRef.current
    if (!scroll) return
    if (event.name === 'pageup') scroll.scrollBy?.(-10)
    if (event.name === 'pagedown') scroll.scrollBy?.(10)
    if (event.name === 'home') scroll.scrollTo?.(0)
    if (event.name === 'end') scroll.scrollToBottom?.()
  })

  const visible = useMemo(
    () => messages.slice(windowStart, windowEnd),
    [messages, windowStart, windowEnd],
  )

  return (
    <box flexDirection="column" width={columns}>
      {windowStart > 0 ? (
        <text fg={c.dim}>
          \u2026 +{windowStart} earlier messages
        </text>
      ) : null}
      {visible.map((msg, offset) => {
        const idx = windowStart + offset
        const k = keysRef.current[idx] ?? String(idx)
        const expanded = isItemExpanded?.(msg) ?? false
        const clickable = isItemClickable ? isItemClickable(msg) : true
        return (
          <box
            key={k}
            flexDirection="column"
            backgroundColor={expanded ? c.userBubbleBg : undefined}
            paddingBottom={expanded ? 1 : 0}
            onMouseDown={
              clickable && onItemClick ? () => onItemClick(msg) : undefined
            }
          >
            {renderItem(msg, idx)}
          </box>
        )
      })}
      {windowEnd < messages.length ? (
        <text fg={c.dim}>
          \u2026 +{messages.length - windowEnd} later messages
        </text>
      ) : null}
    </box>
  )
}
