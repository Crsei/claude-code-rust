import React, { type RefObject, useEffect, useRef } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { matchesShortcut } from '../keybindings.js'
import { useAppState } from '../store/app-store.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/ScrollKeybindingHandler.tsx`.
 *
 * Upstream owns a small empire: wheel acceleration curves (native and
 * xterm.js), drag-to-scroll, OSC 52 clipboard integration, less-style
 * modal pager keys, and cooperative text selection with Ink. None of
 * those scaffolding pieces exist on the Lite frontend — OpenTUI's
 * `<scrollbox>` provides only the handle methods used below.
 *
 * The Lite port keeps the same API surface (scrollRef, isActive,
 * onScroll, isModal) and wires up the subset of keystrokes we can
 * service: PgUp/PgDn (half page), ↑/↓ (line), Home/End (top/bottom),
 * and — when `isModal` — the less-lineage `g`/`G`/`ctrl+u`/`ctrl+d`/
 * `ctrl+b`/`ctrl+f`/`j`/`k`/space/`b` modal pager chords. Wheel and
 * drag handling stay in the framework — OpenTUI already consumes
 * wheel events natively.
 */

// Types kept close to upstream so call sites don't need to rewire.
export type ScrollBoxHandle = {
  scrollBy: (delta: number) => void
  scrollTo: (top: number) => void
  scrollToBottom: () => void
  getScrollTop: () => number
  getScrollHeight: () => number
  getViewportHeight: () => number
  getViewportTop?: () => number
  getPendingDelta?: () => number
}

type Props = {
  scrollRef: RefObject<ScrollBoxHandle | null>
  isActive: boolean
  onScroll?: (sticky: boolean, handle: ScrollBoxHandle) => void
  /** Enables `g`/`G` + `ctrl+u/d/b/f` modal pager chords. Safe only
   *  when no text input is competing for those keys. */
  isModal?: boolean
}

export type ModalPagerAction =
  | 'lineUp'
  | 'lineDown'
  | 'halfPageUp'
  | 'halfPageDown'
  | 'fullPageUp'
  | 'fullPageDown'
  | 'top'
  | 'bottom'

// ---------------------------------------------------------------------------
// Pure helpers (exported for tests — mirrors the upstream surface).
// ---------------------------------------------------------------------------

/**
 * Keyboard page jump. `scrollTo` writes directly, clearing any pending
 * wheel accumulator. Returns true when the target clamps against the
 * bottom — callers use this to re-enable sticky mode.
 */
export function jumpBy(s: ScrollBoxHandle, delta: number): boolean {
  const max = Math.max(0, s.getScrollHeight() - s.getViewportHeight())
  const current = s.getScrollTop() + (s.getPendingDelta?.() ?? 0)
  const target = current + delta
  if (target >= max) {
    s.scrollTo(max)
    s.scrollToBottom()
    return true
  }
  s.scrollTo(Math.max(0, target))
  return false
}

export function scrollDown(s: ScrollBoxHandle, amount: number): boolean {
  const max = Math.max(0, s.getScrollHeight() - s.getViewportHeight())
  const effective = s.getScrollTop() + (s.getPendingDelta?.() ?? 0)
  if (effective + amount >= max) {
    s.scrollToBottom()
    return true
  }
  s.scrollBy(amount)
  return false
}

export function scrollUp(s: ScrollBoxHandle, amount: number): void {
  const effective = s.getScrollTop() + (s.getPendingDelta?.() ?? 0)
  if (effective - amount <= 0) {
    s.scrollTo(0)
    return
  }
  s.scrollBy(-amount)
}

type PagerKeyInput = {
  ctrl?: boolean
  meta?: boolean
  shift?: boolean
  upArrow?: boolean
  downArrow?: boolean
  home?: boolean
  end?: boolean
}

/**
 * Maps a keystroke to a modal pager action. Follows the less lineage
 * (`ctrl+u/d/b/f`, `g`/`G`, space, `b`) plus the j/k/↑/↓ re-add Tom
 * requested in the upstream thread. Returns null for keys that aren't
 * pager chords so callers can fall through.
 */
export function modalPagerAction(
  input: string,
  key: PagerKeyInput,
): ModalPagerAction | null {
  if (key.meta) return null
  if (!key.ctrl && !key.shift) {
    if (key.upArrow) return 'lineUp'
    if (key.downArrow) return 'lineDown'
    if (key.home) return 'top'
    if (key.end) return 'bottom'
  }
  if (key.ctrl) {
    if (key.shift) return null
    switch (input) {
      case 'u':
        return 'halfPageUp'
      case 'd':
        return 'halfPageDown'
      case 'b':
        return 'fullPageUp'
      case 'f':
        return 'fullPageDown'
      case 'n':
        return 'lineDown'
      case 'p':
        return 'lineUp'
      default:
        return null
    }
  }
  const c = input[0]
  if (!c || input !== c.repeat(input.length)) return null
  if (c === 'G' || (c === 'g' && key.shift)) return 'bottom'
  if (key.shift) return null
  switch (c) {
    case 'g':
      return 'top'
    case 'j':
      return 'lineDown'
    case 'k':
      return 'lineUp'
    case ' ':
      return 'fullPageDown'
    case 'b':
      return 'fullPageUp'
    default:
      return null
  }
}

/**
 * Applies a pager action to a ScrollBox handle. Returns the resulting
 * sticky state, or `null` when `act === null`.
 */
export function applyModalPagerAction(
  s: ScrollBoxHandle,
  act: ModalPagerAction | null,
): boolean | null {
  switch (act) {
    case null:
      return null
    case 'lineUp':
      return jumpBy(s, -1)
    case 'lineDown':
      return jumpBy(s, 1)
    case 'halfPageUp':
    case 'halfPageDown': {
      const half = Math.max(1, Math.floor(s.getViewportHeight() / 2))
      return jumpBy(s, act === 'halfPageDown' ? half : -half)
    }
    case 'fullPageUp':
    case 'fullPageDown': {
      const page = Math.max(1, s.getViewportHeight())
      return jumpBy(s, act === 'fullPageDown' ? page : -page)
    }
    case 'top':
      s.scrollTo(0)
      return false
    case 'bottom': {
      const max = Math.max(0, s.getScrollHeight() - s.getViewportHeight())
      s.scrollTo(max)
      s.scrollToBottom()
      return true
    }
  }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

function toShortcutKey(event: KeyEvent) {
  const name = event.name ?? ''
  return {
    ctrl: event.ctrl ?? false,
    meta: event.meta ?? false,
    shift: event.shift ?? false,
    pageUp: name === 'pageup',
    pageDown: name === 'pagedown',
    upArrow: name === 'up',
    downArrow: name === 'down',
    home: name === 'home',
    end: name === 'end',
  }
}

export function ScrollKeybindingHandler({
  scrollRef,
  isActive,
  onScroll,
  isModal = false,
}: Props): React.ReactNode {
  const { keybindingConfig } = useAppState()
  const onScrollRef = useRef(onScroll)
  onScrollRef.current = onScroll

  useEffect(() => () => void 0, [])

  useKeyboard((event: KeyEvent) => {
    if (!isActive || event.eventType === 'release') return
    const s = scrollRef.current
    if (!s) return

    const key = toShortcutKey(event)
    const name = event.name
    const input =
      event.sequence && event.sequence.length <= 4 ? event.sequence : name ?? ''

    const runBinding = (
      action: string,
      ctx: string,
      handler: (handle: ScrollBoxHandle) => boolean,
    ): boolean => {
      if (!matchesShortcut(action, '', key, name, { context: ctx, config: keybindingConfig })) {
        return false
      }
      const sticky = handler(s)
      onScrollRef.current?.(sticky, s)
      return true
    }

    if (
      runBinding('scroll:pageUp', 'Scroll', h => {
        const d = -Math.max(1, Math.floor(h.getViewportHeight() / 2))
        return jumpBy(h, d)
      }) ||
      runBinding('scroll:pageDown', 'Scroll', h => {
        const d = Math.max(1, Math.floor(h.getViewportHeight() / 2))
        return jumpBy(h, d)
      }) ||
      runBinding('scroll:lineUp', 'Scroll', h => {
        scrollUp(h, 3)
        return false
      }) ||
      runBinding('scroll:lineDown', 'Scroll', h => scrollDown(h, 3)) ||
      runBinding('scroll:top', 'Scroll', h => {
        h.scrollTo(0)
        return false
      }) ||
      runBinding('scroll:bottom', 'Scroll', h => {
        const max = Math.max(0, h.getScrollHeight() - h.getViewportHeight())
        h.scrollTo(max)
        h.scrollToBottom()
        return true
      }) ||
      runBinding('scroll:halfPageUp', 'Scroll', h =>
        jumpBy(h, -Math.max(1, Math.floor(h.getViewportHeight() / 2))),
      ) ||
      runBinding('scroll:halfPageDown', 'Scroll', h =>
        jumpBy(h, Math.max(1, Math.floor(h.getViewportHeight() / 2))),
      ) ||
      runBinding('scroll:fullPageUp', 'Scroll', h =>
        jumpBy(h, -Math.max(1, h.getViewportHeight())),
      ) ||
      runBinding('scroll:fullPageDown', 'Scroll', h =>
        jumpBy(h, Math.max(1, h.getViewportHeight())),
      )
    ) {
      return
    }

    if (isModal) {
      const act = modalPagerAction(input, key)
      if (act) {
        const sticky = applyModalPagerAction(s, act)
        if (sticky !== null) onScrollRef.current?.(sticky, s)
      }
    }
  })

  return null
}
