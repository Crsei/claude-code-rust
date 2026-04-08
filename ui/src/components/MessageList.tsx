import React, { useCallback, useEffect, useRef, useState } from 'react'
import {
  Box,
  Text,
  ScrollBox,
  VirtualList,
  type ClickEvent,
  type ScrollBoxHandle,
  type VirtualScrollResult,
  useInput,
} from 'ink-terminal'
import { Markdown } from 'ink-terminal/markdown'
import { useAppState } from '../store/app-store.js'
import { MessageBubble } from './MessageBubble.js'
import type { UIMessage } from '../store/app-store.js'

type ScrollMetrics = {
  top: number
  viewport: number
  height: number
}

const INITIAL_SCROLL_METRICS: ScrollMetrics = {
  top: 0,
  viewport: 0,
  height: 0,
}

interface MessageListProps {
  isActive?: boolean
  onActivate?: () => void
}

export function MessageList({ isActive = true, onActivate }: MessageListProps) {
  const scrollRef = useRef<ScrollBoxHandle>(null)
  const resultRef = useRef<VirtualScrollResult | null>(null)
  const { messages, isStreaming, streamingText } = useAppState()
  const [scrollMetrics, setScrollMetrics] = useState<ScrollMetrics>(INITIAL_SCROLL_METRICS)

  const items: Array<UIMessage | { id: string; role: 'streaming'; content: string; timestamp: number }> = [...messages]

  if (isStreaming && streamingText) {
    items.push({
      id: 'streaming-partial',
      role: 'streaming' as any,
      content: streamingText,
      timestamp: Date.now(),
    })
  }

  const columns = process.stdout.columns ?? 80
  const itemCount = items.length

  const refreshScrollMetrics = useCallback(() => {
    const scroll = scrollRef.current
    if (!scroll) {
      setScrollMetrics(INITIAL_SCROLL_METRICS)
      return
    }

    const nextMetrics: ScrollMetrics = {
      top: Math.max(0, Math.floor(scroll.getScrollTop() + scroll.getPendingDelta())),
      viewport: Math.max(0, Math.floor(scroll.getViewportHeight())),
      height: Math.max(0, Math.floor(scroll.getFreshScrollHeight())),
    }

    setScrollMetrics(prev => {
      if (
        prev.top === nextMetrics.top
        && prev.viewport === nextMetrics.viewport
        && prev.height === nextMetrics.height
      ) {
        return prev
      }
      return nextMetrics
    })
  }, [])

  const scrollByViewport = useCallback((direction: 1 | -1) => {
    const scroll = scrollRef.current
    if (!scroll) return
    const step = Math.max(1, Math.floor(scroll.getViewportHeight() * 0.8))
    scroll.scrollBy(direction * step)
    refreshScrollMetrics()
  }, [refreshScrollMetrics])

  useEffect(() => {
    refreshScrollMetrics()
  }, [refreshScrollMetrics, itemCount, columns, isStreaming])

  useInput((_, key, event) => {
    if (itemCount === 0) return
    const scroll = scrollRef.current
    if (!scroll) return

    if (key.wheelUp) {
      scroll.scrollBy(-3)
      refreshScrollMetrics()
      event.stopImmediatePropagation()
      return
    }

    if (key.wheelDown) {
      scroll.scrollBy(3)
      refreshScrollMetrics()
      event.stopImmediatePropagation()
      return
    }

    if (key.pageUp) {
      scrollByViewport(-1)
      event.stopImmediatePropagation()
      return
    }

    if (key.pageDown) {
      scrollByViewport(1)
      event.stopImmediatePropagation()
      return
    }

    if (key.ctrl && key.upArrow) {
      scroll.scrollBy(-3)
      refreshScrollMetrics()
      event.stopImmediatePropagation()
      return
    }

    if (key.ctrl && key.downArrow) {
      scroll.scrollBy(3)
      refreshScrollMetrics()
      event.stopImmediatePropagation()
      return
    }

    if (key.ctrl && key.home) {
      scroll.scrollTo(0)
      refreshScrollMetrics()
      event.stopImmediatePropagation()
      return
    }

    if (key.ctrl && key.end) {
      scroll.scrollToBottom()
      refreshScrollMetrics()
      event.stopImmediatePropagation()
    }
  }, { isActive: isActive && itemCount > 0 })

  const maxTop = Math.max(0, scrollMetrics.height - scrollMetrics.viewport)
  const canScrollUp = scrollMetrics.top > 0
  const canScrollDown = scrollMetrics.top < maxTop
  const hasOverflow = maxTop > 0

  return (
    <Box
      flexDirection="column"
      flexGrow={1}
      onClick={(event: ClickEvent) => {
        onActivate?.()
        event.stopImmediatePropagation()
      }}
    >
      <ScrollBox ref={scrollRef} stickyScroll flexGrow={1}>
        {items.length > 0 ? (
          <VirtualList
            items={items}
            scrollRef={scrollRef}
            columns={columns}
            itemKey={(item) => item.id}
            resultRef={resultRef}
            renderItem={(item) => {
              if (item.id === 'streaming-partial') {
                return (
                  <Box flexDirection="column" paddingX={1} marginBottom={1}>
                    <Text color="ansi:magenta" bold>Assistant</Text>
                    <Box paddingLeft={2} flexDirection="column">
                      <Text>{item.content}</Text>
                    </Box>
                  </Box>
                )
              }
              return <MessageBubble msg={item as UIMessage} />
            }}
          />
        ) : null}
      </ScrollBox>
      {hasOverflow && (
        <Box paddingX={1}>
          <Text dim>
            {canScrollUp ? '^' : '.'} Scroll {Math.min(scrollMetrics.top, maxTop)}/{maxTop} {canScrollDown ? 'v' : '.'} (wheel, PgUp/PgDn, Ctrl+Up/Down, Ctrl+Home/End)
          </Text>
        </Box>
      )}
    </Box>
  )
}
