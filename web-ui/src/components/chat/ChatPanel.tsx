import { useEffect, useRef } from 'react'
import { MessageList } from './MessageList'
import { InputBar } from './InputBar'
import { StreamingText } from './StreamingText'
import { StreamingBlocks } from './StreamingBlocks'
import { useChat } from '@/hooks/useChat'
import { useChatStore } from '@/lib/store'

export function ChatPanel() {
  const { messages, isStreaming, streamingContent, sendMessage, abort } = useChat()
  const streamingBlocks = useChatStore((s) => s.streamingBlocks)
  const scrollRef = useRef<HTMLDivElement>(null)
  const scrollAreaRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom when messages change or streaming content updates
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages, streamingContent, streamingBlocks])

  // Determine which streaming view to show
  const hasStreamingBlocks = isStreaming && streamingBlocks.filter(Boolean).length > 0
  const hasPlainStreaming = isStreaming && streamingContent && !hasStreamingBlocks

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div ref={scrollAreaRef} className="flex-1 overflow-y-auto px-4 py-6">
        <div className="mx-auto max-w-3xl space-y-4">
          {messages.length === 0 && !isStreaming && (
            <div className="flex flex-col items-center justify-center py-20 text-muted-foreground">
              <h2 className="text-lg font-medium">cc-rust Web UI</h2>
              <p className="mt-1 text-sm">Start a conversation to begin.</p>
            </div>
          )}
          <MessageList messages={messages} />
          {hasStreamingBlocks && (
            <StreamingBlocks blocks={streamingBlocks} />
          )}
          {hasPlainStreaming && (
            <StreamingText content={streamingContent} />
          )}
          {/* Scroll anchor */}
          <div ref={scrollRef} />
        </div>
      </div>
      <InputBar
        onSend={sendMessage}
        onAbort={abort}
        isStreaming={isStreaming}
      />
    </div>
  )
}
