import React, {
  createContext,
  type ReactNode,
  type RefObject,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useTerminalDimensions } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/FullscreenLayout.tsx`.
 *
 * Upstream's layout combines several pieces the Ink runtime ships:
 *  - alternate-screen / DECSTBM-aware `ScrollBox` with sticky-scroll.
 *  - `StickyPromptHeader` — pinned current-turn breadcrumb.
 *  - `NewMessagesPill` — floating jump-to-bottom pill.
 *  - `PromptOverlayProvider` — portal for slash-command suggestions.
 *  - `ModalContext` — a pane that can paint over the scrollbox.
 *
 * OpenTUI's `<box>` primitives do not offer sticky-scroll or absolute
 * overlays in the same shape, so this Lite variant keeps the three
 * public APIs upstream exposes and reimplements them with the tools
 * OpenTUI provides:
 *
 * 1. `FullscreenLayout` — a flex column that stacks the scrollable,
 *    optional overlay, optional bottom-float, bottom slot, and
 *    optional modal pane. Fullscreen gate derived from env vars.
 * 2. `useUnseenDivider(messageCount)` — pure hook for the "N new
 *    messages" divider, tracking `dividerIndex` + a `dividerYRef` that
 *    external scroll widgets can compare against. Kept identical to
 *    upstream so callers can port without behavior drift.
 * 3. `computeUnseenDivider` + `countUnseenAssistantTurns` — pure helpers
 *    used by the pill's count label, exported verbatim so callers that
 *    already consume upstream's helpers keep working.
 *
 * The layout does not attempt to reproduce DECSTBM scroll regions or
 * OSC 8 hyperlink interception — the Lite frontend uses native terminal
 * scrolling, and OSC 8 clicks are handled by the terminal directly.
 */

export type StickyPrompt = { text: string; scrollTo: () => void } | 'clicked'

/**
 * Context that child scroll-trackers use to update the sticky-prompt
 * header. Upstream's `StickyTracker` component calls
 * `setStickyPrompt({text, scrollTo})` on scroll-away and `null` on
 * repin; we preserve the signature so the port stays drop-in.
 */
export const ScrollChromeContext = createContext<{
  setStickyPrompt: (p: StickyPrompt | null) => void
}>({ setStickyPrompt: () => {} })

/** Access the chrome context from inside a descendant. */
export function useScrollChrome() {
  return useContext(ScrollChromeContext)
}

// ---------------------------------------------------------------------------
// `useUnseenDivider` — upstream's divider state machine, rehosted on
// OpenTUI-friendly types. The caller owns the scroll handle; we keep the
// same minimal interface so swapping implementations does not affect
// call sites.
// ---------------------------------------------------------------------------

export interface ScrollHandle {
  getScrollTop(): number
  getScrollHeight(): number
  getViewportHeight(): number
  /** Delta of `scrollBy` calls that haven't been flushed yet. */
  getPendingDelta(): number
  scrollToBottom(): void
}

export interface UnseenDivider<Uuid = string> {
  firstUnseenUuid: Uuid
  count: number
}

export interface UnseenDividerMessage<Uuid = string> {
  uuid: Uuid
  /** Discriminant used by the helpers — matches upstream. */
  type: 'user' | 'assistant' | 'progress' | 'attachment' | 'tool_result' | 'system'
  /** Present on assistant turns — drives "has visible text" heuristic. */
  assistantText?: string
  /** Present for attachments that render nothing. */
  nullRendering?: boolean
}

export function useUnseenDivider<Uuid extends string>(messageCount: number): {
  dividerIndex: number | null
  dividerYRef: RefObject<number | null>
  onScrollAway: (handle: ScrollHandle) => void
  onRepin: () => void
  jumpToNew: (handle: ScrollHandle | null) => void
  shiftDivider: (indexDelta: number, heightDelta: number) => void
} {
  const [dividerIndex, setDividerIndex] = useState<number | null>(null)
  const countRef = useRef(messageCount)
  countRef.current = messageCount
  const dividerYRef = useRef<number | null>(null)

  const onRepin = useCallback(() => {
    setDividerIndex(null)
  }, [])

  const onScrollAway = useCallback((handle: ScrollHandle) => {
    const max = Math.max(
      0,
      handle.getScrollHeight() - handle.getViewportHeight(),
    )
    if (handle.getScrollTop() + handle.getPendingDelta() >= max) return
    if (dividerYRef.current === null) {
      dividerYRef.current = handle.getScrollHeight()
      setDividerIndex(countRef.current)
    }
  }, [])

  const jumpToNew = useCallback((handle: ScrollHandle | null) => {
    if (!handle) return
    handle.scrollToBottom()
  }, [])

  useEffect(() => {
    if (dividerIndex === null) {
      dividerYRef.current = null
    } else if (messageCount < dividerIndex) {
      dividerYRef.current = null
      setDividerIndex(null)
    }
  }, [messageCount, dividerIndex])

  const shiftDivider = useCallback(
    (indexDelta: number, heightDelta: number) => {
      setDividerIndex(idx => (idx === null ? null : idx + indexDelta))
      if (dividerYRef.current !== null) {
        dividerYRef.current += heightDelta
      }
    },
    [],
  )

  return {
    dividerIndex,
    dividerYRef,
    onScrollAway,
    onRepin,
    jumpToNew,
    shiftDivider,
  }
}

/**
 * Count assistant turns past `dividerIndex`. Matches upstream:
 * - Progress entries are skipped.
 * - Tool-use-only assistant entries are skipped (no visible text).
 * - A non-assistant → assistant transition counts as one turn.
 */
export function countUnseenAssistantTurns<Uuid>(
  messages: readonly UnseenDividerMessage<Uuid>[],
  dividerIndex: number,
): number {
  let count = 0
  let prevWasAssistant = false
  for (let i = dividerIndex; i < messages.length; i++) {
    const m = messages[i]
    if (!m) continue
    if (m.type === 'progress') continue
    if (m.type === 'assistant' && !hasVisibleText(m)) continue
    const isAssistant = m.type === 'assistant'
    if (isAssistant && !prevWasAssistant) count++
    prevWasAssistant = isAssistant
  }
  return count
}

function hasVisibleText<Uuid>(m: UnseenDividerMessage<Uuid>): boolean {
  if (m.type !== 'assistant') return false
  return typeof m.assistantText === 'string' && m.assistantText.trim() !== ''
}

/**
 * Build the unseenDivider payload for the pill + divider line.
 * Matches upstream: returns `undefined` when no content exists past
 * `dividerIndex`; otherwise floors `count` at 1 so the pill flips to
 * "1 new message" as soon as something lands.
 */
export function computeUnseenDivider<Uuid>(
  messages: readonly UnseenDividerMessage<Uuid>[],
  dividerIndex: number | null,
): UnseenDivider<Uuid> | undefined {
  if (dividerIndex === null) return undefined
  let anchorIdx = dividerIndex
  while (
    anchorIdx < messages.length &&
    (messages[anchorIdx]?.type === 'progress' ||
      messages[anchorIdx]?.nullRendering === true)
  ) {
    anchorIdx++
  }
  const anchor = messages[anchorIdx]
  if (!anchor) return undefined
  const count = countUnseenAssistantTurns(messages, dividerIndex)
  return { firstUnseenUuid: anchor.uuid, count: Math.max(1, count) }
}

// ---------------------------------------------------------------------------
// FullscreenLayout
// ---------------------------------------------------------------------------

/**
 * Upstream gates fullscreen via `isFullscreenEnvEnabled`. cc-rust has no
 * ant-only env block, so we mirror the two observable opt-outs and
 * default to on: `CLAUDE_CODE_NO_FLICKER=1` opts out, anything else —
 * including unset — opts in.
 */
function isFullscreenEnvEnabled(): boolean {
  const env = (globalThis as unknown as { process?: { env?: Record<string, string | undefined> } }).process?.env
  if (!env) return true
  return env.CLAUDE_CODE_NO_FLICKER !== '1'
}

const MODAL_TRANSCRIPT_PEEK = 2

type PillProps = {
  count: number
  onClick?: () => void
}

type Props = {
  /** Content that scrolls (messages, tool output). */
  scrollable: ReactNode
  /** Content pinned to the bottom (spinner, prompt, permissions). */
  bottom: ReactNode
  /** Rendered inside the scroll area after messages. */
  overlay?: ReactNode
  /** Absolute-positioned bottom-right floating node (companion speech
   *  bubble in upstream). Fullscreen only. */
  bottomFloat?: ReactNode
  /** Slash-command / dialog content rendered above the bottom slot. */
  modal?: ReactNode
  /** Force-hide the pill (e.g. sub-agent task view). */
  hidePill?: boolean
  /** Force-hide the sticky prompt header (e.g. teammate-task view). */
  hideSticky?: boolean
  /** Value for the pill text — 0 renders "Jump to bottom". */
  newMessageCount?: number
  /** Called when the user clicks the "N new" pill. */
  onPillClick?: () => void
  /** When true, always render the pill (used by tests/snapshots). */
  forcePillVisible?: boolean
  /** When set, renders a suggestions overlay above the bottom slot. */
  suggestionsOverlay?: ReactNode
  /** When set, renders a dialog overlay (portaled from PromptInput). */
  dialogOverlay?: ReactNode
}

export function FullscreenLayout({
  scrollable,
  bottom,
  overlay,
  bottomFloat,
  modal,
  hidePill = false,
  hideSticky = false,
  newMessageCount = 0,
  onPillClick,
  forcePillVisible = false,
  suggestionsOverlay,
  dialogOverlay,
}: Props) {
  const { width, height } = useTerminalDimensions()
  const [stickyPrompt, setStickyPrompt] = useState<StickyPrompt | null>(null)
  const chromeCtx = useMemo(() => ({ setStickyPrompt }), [])

  if (!isFullscreenEnvEnabled()) {
    // Non-fullscreen: render content sequentially — matches upstream.
    return (
      <box flexDirection="column" width="100%" height="100%">
        {scrollable}
        {bottom}
        {overlay}
        {modal}
      </box>
    )
  }

  const activeSticky = hideSticky ? null : stickyPrompt
  const headerPrompt =
    activeSticky != null && activeSticky !== 'clicked' && overlay == null
      ? activeSticky
      : null
  const padCollapsed = activeSticky != null && overlay == null

  const pillVisible =
    !hidePill && overlay == null && (forcePillVisible || newMessageCount > 0)

  return (
    <ScrollChromeContext.Provider value={chromeCtx}>
      <box flexDirection="row" flexGrow={1} width={width} height={height}>
        <box flexDirection="column" flexGrow={1} width={width}>
          <box flexGrow={1} flexDirection="column">
            {headerPrompt && (
              <StickyPromptHeader
                text={headerPrompt.text}
                onClick={headerPrompt.scrollTo}
              />
            )}
            <box
              flexGrow={1}
              flexDirection="column"
              paddingTop={padCollapsed ? 0 : 1}
            >
              {scrollable}
              {overlay}
            </box>
            {pillVisible && (
              <NewMessagesPill count={newMessageCount} onClick={onPillClick} />
            )}
            {bottomFloat != null && (
              <box position="absolute" bottom={0} right={0}>
                {bottomFloat}
              </box>
            )}
          </box>
          <box flexDirection="column" flexShrink={0} width={width}>
            {suggestionsOverlay && (
              <box paddingX={2} paddingTop={1}>
                {suggestionsOverlay}
              </box>
            )}
            {dialogOverlay}
            <box flexDirection="column" width={width} flexGrow={1}>
              {bottom}
            </box>
          </box>
        </box>
      </box>
      {modal != null && (
        <box
          position="absolute"
          bottom={0}
          left={0}
          right={0}
          flexDirection="column"
          backgroundColor={c.bg}
        >
          <box flexShrink={0}>
            <text fg={c.accent}>{'\u2594'.repeat(Math.max(1, width))}</text>
          </box>
          <box
            flexDirection="column"
            paddingX={2}
            flexShrink={0}
            maxHeight={Math.max(4, height - MODAL_TRANSCRIPT_PEEK - 1)}
          >
            {modal}
          </box>
        </box>
      )}
    </ScrollChromeContext.Provider>
  )
}

function NewMessagesPill({ count, onClick: _onClick }: PillProps) {
  // OpenTUI's `<box>` does not expose a mouse `onClick`. We preserve
  // upstream's signature so callers can still hand us the handler; the
  // Lite frontend surfaces the pill as a visual cue and relies on the
  // dedicated "jump to bottom" keyboard shortcut to trigger the action.
  return (
    <box
      position="absolute"
      bottom={0}
      left={0}
      right={0}
      justifyContent="center"
      flexDirection="row"
    >
      <box backgroundColor={c.userBubbleBg} paddingX={1}>
        <text fg={c.dim}>
          {count > 0 ? `${count} new message${count === 1 ? '' : 's'}` : 'Jump to bottom'}
          {' \u2193 '}
        </text>
      </box>
    </box>
  )
}

function StickyPromptHeader({
  text,
  onClick: _onClick,
}: {
  text: string
  onClick: () => void
}) {
  // Same caveat as `NewMessagesPill` — the click is informational, not
  // wired to OpenTUI's box.
  return (
    <box
      flexShrink={0}
      width="100%"
      height={1}
      paddingRight={1}
      backgroundColor={c.userBubbleBg}
    >
      <text fg={c.dim}>
        {'\u276F '}
        {text}
      </text>
    </box>
  )
}
