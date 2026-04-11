/**
 * ink-terminal → OpenTUI compatibility layer
 *
 * Provides ink-terminal-compatible component and hook APIs
 * backed by @opentui/react + @opentui/core.
 *
 * This file is the ONLY place that imports from @opentui/*.
 * All component files import from here instead.
 */
import React, { createContext, useContext, useCallback, useEffect, useRef, useState } from 'react'
import { useKeyboard, useRenderer, useTimeline, useOnResize, useTerminalDimensions } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'

// ─── Color mapping ───────────────────────────────��────────────────

const ANSI_COLORS: Record<string, string> = {
  'ansi:black': '#000000',
  'ansi:red': '#CC0000',
  'ansi:green': '#4EC940',
  'ansi:yellow': '#C4A500',
  'ansi:blue': '#3D6DCC',
  'ansi:magenta': '#CC00CC',
  'ansi:cyan': '#00AAAA',
  'ansi:white': '#CCCCCC',
  'ansi:blackBright': '#666666',
  'ansi:redBright': '#FF5555',
  'ansi:greenBright': '#55FF55',
  'ansi:yellowBright': '#FFFF55',
  'ansi:blueBright': '#5555FF',
  'ansi:magentaBright': '#FF55FF',
  'ansi:cyanBright': '#55FFFF',
  'ansi:whiteBright': '#FFFFFF',
}

function mapColor(color?: string): string | undefined {
  if (!color) return undefined
  return ANSI_COLORS[color] ?? color
}

// ─── Text component ──────────────────────────────────��────────────

const TextNestingCtx = createContext(false)

interface TextProps {
  children?: React.ReactNode
  bold?: boolean
  dim?: boolean
  italic?: boolean
  inverse?: boolean
  color?: string
  wrap?: string
  dimColor?: boolean
  [key: string]: any
}

export function Text({ children, bold, dim, italic, inverse, color, wrap, dimColor, ...rest }: TextProps) {
  const inside = useContext(TextNestingCtx)

  let fg = mapColor(color)
  let bg: string | undefined

  // Approximate dim with gray when no explicit color
  if (dim && !fg) fg = '#888888'
  if (dimColor && !fg) fg = '#888888'

  // inverse: swap fg/bg as rough approximation
  if (inverse) {
    bg = fg || '#CCCCCC'
    fg = '#000000'
  }

  let content: React.ReactNode = children
  if (bold) content = <strong>{content}</strong>
  if (italic) content = <em>{content}</em>

  if (inside) {
    // Nested Text → render as <span>
    if (fg || bg) return <span fg={fg} bg={bg}>{content}</span>
    return <>{content}</>
  }

  return (
    <TextNestingCtx.Provider value={true}>
      <text fg={fg} bg={bg}>{content}</text>
    </TextNestingCtx.Provider>
  )
}

// ─── Box component ────────────────────────────────────────────────

interface BoxProps {
  children?: React.ReactNode
  flexDirection?: 'row' | 'column' | 'row-reverse' | 'column-reverse'
  flexGrow?: number
  alignItems?: string
  justifyContent?: string
  gap?: number
  width?: number | string
  height?: number | string
  minWidth?: number
  maxWidth?: number
  padding?: number
  paddingX?: number
  paddingY?: number
  paddingLeft?: number
  paddingRight?: number
  paddingTop?: number
  paddingBottom?: number
  margin?: number
  marginTop?: number
  marginBottom?: number
  marginLeft?: number
  marginRight?: number
  borderStyle?: string
  borderColor?: string
  borderDimColor?: boolean
  borderTop?: boolean
  borderBottom?: boolean
  borderLeft?: boolean
  borderRight?: boolean
  border?: boolean
  position?: string
  top?: number
  bottom?: number
  left?: number
  right?: number
  onClick?: (event: any) => void
  ref?: React.Ref<any>
  [key: string]: any
}

export const Box = React.forwardRef<any, BoxProps>(function Box(
  { children, borderStyle, borderDimColor, onClick, ...rest },
  ref
) {
  // Map borderStyle names
  let mappedBorder = borderStyle
  if (borderStyle === 'round') mappedBorder = 'rounded'
  if (borderStyle === 'single') mappedBorder = 'single'

  // Map borderDimColor to a gray border color
  const extraProps: Record<string, any> = {}
  if (borderDimColor) extraProps.borderColor = '#666666'

  // Map onClick → onMouseDown
  if (onClick) extraProps.onMouseDown = onClick

  // If any border side prop is explicitly true without borderStyle, set border
  if (mappedBorder) extraProps.borderStyle = mappedBorder

  return (
    <box ref={ref} {...rest} {...extraProps}>
      {children}
    </box>
  )
})

// ─── Spacer ───────────────────────────────────────────────────────

export function Spacer() {
  return <box flexGrow={1} />
}

// ─── AlternateScreen ──────────────────────────────────────────────

interface AlternateScreenProps {
  children?: React.ReactNode
  mouseTracking?: boolean
}

export function AlternateScreen({ children }: AlternateScreenProps) {
  // OpenTUI uses alternate screen by default
  return <>{children}</>
}

// ─── ScrollBox ────────────────────────────────────────────────────

export interface ScrollBoxHandle {
  scrollTo(pos: number): void
  scrollBy(amount: number): void
  scrollToBottom(): void
  getScrollTop(): number
  getPendingDelta(): number
  getViewportHeight(): number
  getFreshScrollHeight(): number
}

interface ScrollBoxProps {
  children?: React.ReactNode
  stickyScroll?: boolean
  flexGrow?: number
  width?: string | number
  height?: string | number
  ref?: React.Ref<ScrollBoxHandle>
  [key: string]: any
}

export const ScrollBox = React.forwardRef<ScrollBoxHandle, ScrollBoxProps>(function ScrollBox(
  { children, stickyScroll, ...rest },
  ref
) {
  const innerRef = useRef<any>(null)
  const scrollTopRef = useRef(0)
  const pendingDeltaRef = useRef(0)

  // Expose handle to parent
  React.useImperativeHandle(ref, () => ({
    scrollTo(pos: number) {
      innerRef.current?.scrollTo?.(pos)
      scrollTopRef.current = pos
    },
    scrollBy(amount: number) {
      innerRef.current?.scrollBy?.(amount)
      scrollTopRef.current += amount
      pendingDeltaRef.current = amount
    },
    scrollToBottom() {
      innerRef.current?.scrollToBottom?.()
      scrollTopRef.current = 999999
    },
    getScrollTop() {
      return scrollTopRef.current
    },
    getPendingDelta() {
      const d = pendingDeltaRef.current
      pendingDeltaRef.current = 0
      return d
    },
    getViewportHeight() {
      // Approximate from terminal height
      return process.stdout.rows ?? 24
    },
    getFreshScrollHeight() {
      return (process.stdout.rows ?? 24) * 2
    },
  }))

  // Auto-scroll to bottom on new content if stickyScroll
  const prevChildCountRef = useRef(0)
  useEffect(() => {
    if (stickyScroll) {
      innerRef.current?.scrollToBottom?.()
    }
  })

  return (
    <scrollbox ref={innerRef} focused {...rest}>
      {children}
    </scrollbox>
  )
})

// ─── VirtualList (degraded: renders all items) ────────────────────

export interface VirtualScrollResult {
  visibleRange: [number, number]
}

interface VirtualListProps<T> {
  items: T[]
  scrollRef: React.RefObject<any>
  columns: number
  itemKey: (item: T) => string
  resultRef?: React.MutableRefObject<VirtualScrollResult | null>
  renderItem: (item: T) => React.ReactNode
}

export function VirtualList<T>({ items, itemKey, renderItem }: VirtualListProps<T>) {
  // No virtualization — render all items (acceptable for moderate counts)
  return (
    <>
      {items.map((item) => (
        <React.Fragment key={itemKey(item)}>
          {renderItem(item)}
        </React.Fragment>
      ))}
    </>
  )
}

// ─── Markdown (degraded: plain text) ──────────────────────────────

interface MarkdownProps {
  children: string
}

export function Markdown({ children }: MarkdownProps) {
  return <text>{children}</text>
}

// ─── useInput hook (wraps useKeyboard) ────────────────────────────

interface InkKeyInfo {
  upArrow: boolean
  downArrow: boolean
  leftArrow: boolean
  rightArrow: boolean
  return: boolean
  escape: boolean
  tab: boolean
  backspace: boolean
  delete: boolean
  pageUp: boolean
  pageDown: boolean
  home: boolean
  end: boolean
  ctrl: boolean
  meta: boolean
  shift: boolean
  wheelUp: boolean
  wheelDown: boolean
}

interface InkInputEvent {
  stopImmediatePropagation(): void
}

type InkInputHandler = (input: string, key: InkKeyInfo, event: InkInputEvent) => void

function keyEventToInk(e: KeyEvent): { input: string; key: InkKeyInfo } {
  const name = e.name ?? ''
  const seq = e.sequence ?? ''

  const key: InkKeyInfo = {
    upArrow: name === 'up',
    downArrow: name === 'down',
    leftArrow: name === 'left',
    rightArrow: name === 'right',
    return: name === 'return' || name === 'enter',
    escape: name === 'escape',
    tab: name === 'tab',
    backspace: name === 'backspace',
    delete: name === 'delete',
    pageUp: name === 'pageup' || name === 'page_up',
    pageDown: name === 'pagedown' || name === 'page_down',
    home: name === 'home',
    end: name === 'end',
    ctrl: e.ctrl ?? false,
    meta: e.meta ?? false,
    shift: e.shift ?? false,
    wheelUp: name === 'wheel_up' || name === 'scrollup',
    wheelDown: name === 'wheel_down' || name === 'scrolldown',
  }

  // Extract printable input character
  let input = ''
  if (seq.length === 1 && seq.charCodeAt(0) >= 32) {
    input = seq
  } else if (e.ctrl && name.length === 1) {
    input = name
  } else if (name.length === 1 && !e.ctrl && !e.meta) {
    input = name
  }

  return { input, key }
}

export function useInput(handler: InkInputHandler, options?: { isActive?: boolean }) {
  const handlerRef = useRef(handler)
  handlerRef.current = handler
  const active = options?.isActive ?? true

  useKeyboard((e: KeyEvent) => {
    if (!active) return
    if (e.eventType === 'release') return

    const { input, key } = keyEventToInk(e)
    const event: InkInputEvent = {
      stopImmediatePropagation() {
        // OpenTUI doesn't have direct equivalent; no-op for now
      },
    }
    handlerRef.current(input, key, event)
  })
}

// ─── useApp hook (wraps useRenderer) ──────────────────────────────

export function useApp() {
  const renderer = useRenderer()
  return {
    exit() {
      renderer?.destroy?.()
    },
  }
}

// ─── useAnimationFrame hook ───────────────────────────────────────

/**
 * Returns [ref, time] where:
 * - ref: a React ref to attach to a Box (for compatibility; unused in OpenTUI)
 * - time: elapsed ms since mount, ticking at `intervalMs` (null to pause)
 */
export function useAnimationFrame(intervalMs: number | null): [React.RefObject<any>, number] {
  const ref = useRef<any>(null)
  const [time, setTime] = useState(0)
  const startRef = useRef(Date.now())

  useEffect(() => {
    if (intervalMs === null || intervalMs <= 0) return

    const id = setInterval(() => {
      setTime(Date.now() - startRef.current)
    }, intervalMs)

    return () => clearInterval(id)
  }, [intervalMs])

  return [ref, time]
}

// ─── ClickEvent type ──────────────────────────────────────────────

export type ClickEvent = {
  stopImmediatePropagation(): void
}

// ─── Re-exports for convenience ───────────────────────────────────

export { useOnResize, useTerminalDimensions } from '@opentui/react'
