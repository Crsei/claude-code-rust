import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useKeyboard, useTerminalDimensions } from '@opentui/react'
import { c } from '../theme.js'
import { useAppState } from '../store/app-store.js'
import { MessageBubble } from './MessageBubble.js'
import type { UIMessage } from '../store/app-store.js'

interface MessageListProps {
  isActive?: boolean
  onActivate?: () => void
  children?: React.ReactNode
}

export function MessageList({ isActive = true, onActivate, children }: MessageListProps) {
  const scrollRef = useRef<any>(null)
  const { messages, isStreaming, streamingText } = useAppState()
  const { width: columns } = useTerminalDimensions()

  const items: Array<UIMessage | { id: string; role: 'streaming'; content: string; timestamp: number }> = [...messages]

  if (isStreaming && streamingText) {
    items.push({
      id: 'streaming-partial',
      role: 'streaming' as any,
      content: streamingText,
      timestamp: Date.now(),
    })
  }

  const itemCount = items.length

  // Auto-scroll to bottom when new content arrives
  useEffect(() => {
    scrollRef.current?.scrollToBottom?.()
  }, [itemCount, isStreaming])

  useKeyboard((e) => {
    if (!isActive || itemCount === 0) return
    if (e.eventType === 'release') return
    const scroll = scrollRef.current
    if (!scroll) return

    if (e.name === 'pageup') {
      scroll.scrollBy(-10)
      return
    }
    if (e.name === 'pagedown') {
      scroll.scrollBy(10)
      return
    }
    if (e.ctrl && e.name === 'up') {
      scroll.scrollBy(-3)
      return
    }
    if (e.ctrl && e.name === 'down') {
      scroll.scrollBy(3)
      return
    }
    if (e.ctrl && e.name === 'home') {
      scroll.scrollTo(0)
      return
    }
    if (e.ctrl && e.name === 'end') {
      scroll.scrollToBottom()
      return
    }
  })

  return (
    <box
      flexDirection="column"
      flexGrow={1}
      onMouseDown={() => onActivate?.()}
    >
      <scrollbox ref={scrollRef} focused={isActive} flexGrow={1} width="100%">
        {items.map((item) => {
          if (item.id === 'streaming-partial') {
            return (
              <box key={item.id} flexDirection="column" paddingX={1} marginBottom={1} width="100%">
                <text><strong><span fg={c.accent}>Assistant</span></strong></text>
                <box paddingLeft={2} flexDirection="column" width="100%">
                  <markdown content={item.content} streaming />
                </box>
              </box>
            )
          }
          return (
            <box key={item.id} width="100%">
              <MessageBubble msg={item as UIMessage} />
            </box>
          )
        })}
        {children}
      </scrollbox>
    </box>
  )
}
