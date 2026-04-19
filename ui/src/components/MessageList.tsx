import React, { useEffect, useMemo, useRef } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { messageScrollHint, matchesShortcut, shortcutLabel, type ViewMode } from '../keybindings.js'
import { useAppState } from '../store/app-store.js'
import { buildRenderItems } from '../store/message-model.js'
import { MessageBubble } from './MessageBubble.js'

interface MessageListProps {
  isActive?: boolean
  onActivate?: () => void
  children?: React.ReactNode
  viewMode: ViewMode
}

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

export function MessageList({
  isActive = true,
  onActivate,
  children,
  viewMode,
}: MessageListProps) {
  const scrollRef = useRef<any>(null)
  const { keybindingConfig, messages, isStreaming, isWaiting, streamingText, streamingThinking } = useAppState()

  const items = useMemo(
    () => buildRenderItems(messages, {
      viewMode,
      isBusy: isStreaming || isWaiting,
      streamingText,
      streamingThinking,
    }),
    [isStreaming, isWaiting, messages, streamingText, streamingThinking, viewMode],
  )

  const itemCount = items.length

  useEffect(() => {
    if (viewMode === 'prompt') {
      scrollRef.current?.scrollToBottom?.()
    }
  }, [itemCount, isStreaming, viewMode])

  useKeyboard((event: KeyEvent) => {
    if (!isActive || itemCount === 0 || event.eventType === 'release') {
      return
    }

    const scroll = scrollRef.current
    if (!scroll) {
      return
    }

    const key = toShortcutKey(event)
    const name = event.name

    if (matchesShortcut('scroll:pageUp', '', key, name, { context: 'Scroll', config: keybindingConfig })) {
      scroll.scrollBy(-10)
      return
    }
    if (matchesShortcut('scroll:pageDown', '', key, name, { context: 'Scroll', config: keybindingConfig })) {
      scroll.scrollBy(10)
      return
    }
    if (matchesShortcut('scroll:lineUp', '', key, name, { context: 'Scroll', config: keybindingConfig })) {
      scroll.scrollBy(-3)
      return
    }
    if (matchesShortcut('scroll:lineDown', '', key, name, { context: 'Scroll', config: keybindingConfig })) {
      scroll.scrollBy(3)
      return
    }
    if (matchesShortcut('scroll:top', '', key, name, { context: 'Scroll', config: keybindingConfig })) {
      scroll.scrollTo(0)
      return
    }
    if (matchesShortcut('scroll:bottom', '', key, name, { context: 'Scroll', config: keybindingConfig })) {
      scroll.scrollToBottom()
    }
  })

  const title = viewMode === 'transcript'
    ? `Transcript | ${shortcutLabel('app:toggleTranscript', { context: 'Global', config: keybindingConfig })} prompt | ${shortcutLabel('transcript:exit', { context: 'Transcript', config: keybindingConfig })} exit`
    : 'Conversation'

  return (
    <box
      flexDirection="column"
      flexGrow={1}
      border
      borderStyle="rounded"
      borderColor="#45475A"
      title={title}
      titleAlignment="left"
      onMouseDown={() => onActivate?.()}
    >
      <box paddingX={1}>
        <text fg="#666666">{messageScrollHint(keybindingConfig)}</text>
      </box>
      <scrollbox ref={scrollRef} focused={isActive} flexGrow={1} width="100%">
        {items.map(item => (
          <box key={item.id} width="100%">
            <MessageBubble item={item} viewMode={viewMode} />
          </box>
        ))}
        {children}
      </scrollbox>
    </box>
  )
}
