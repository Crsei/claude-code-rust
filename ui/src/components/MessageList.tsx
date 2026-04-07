import React, { useRef } from 'react'
import { Box, Text, ScrollBox, VirtualList, type ScrollBoxHandle, type VirtualScrollResult } from 'ink-terminal'
import { Markdown } from 'ink-terminal/markdown'
import { useAppState } from '../store/app-store.js'
import { MessageBubble } from './MessageBubble.js'
import type { UIMessage } from '../store/app-store.js'

export function MessageList() {
  const scrollRef = useRef<ScrollBoxHandle>(null)
  const resultRef = useRef<VirtualScrollResult | null>(null)
  const { messages, isStreaming, streamingText } = useAppState()

  // Build items list: messages + optional streaming partial
  const items: Array<UIMessage | { id: string; role: 'streaming'; content: string; timestamp: number }> = [
    ...messages,
  ]

  if (isStreaming && streamingText) {
    items.push({
      id: 'streaming-partial',
      role: 'streaming' as any,
      content: streamingText,
      timestamp: Date.now(),
    })
  }

  const columns = process.stdout.columns ?? 80

  return (
    <ScrollBox ref={scrollRef} stickyScroll>
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
                    <Markdown streaming>{item.content}</Markdown>
                  </Box>
                </Box>
              )
            }
            return <MessageBubble msg={item as UIMessage} />
          }}
        />
      ) : null}
      {/* Spinner moved to App-level thinking indicator */}
    </ScrollBox>
  )
}
