import { MessageList } from './MessageList'
import { InputBar } from './InputBar'
import { StreamingText } from './StreamingText'
import { useChat } from '@/hooks/useChat'

export function ChatPanel() {
  const { messages, isStreaming, streamingContent, sendMessage, abort } = useChat()

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-4 py-6">
        <div className="mx-auto max-w-3xl space-y-6">
          {messages.length === 0 && !isStreaming && (
            <div className="flex flex-col items-center justify-center py-20 text-muted-foreground">
              <h2 className="text-lg font-medium">cc-rust Web UI</h2>
              <p className="mt-1 text-sm">Start a conversation to begin.</p>
            </div>
          )}
          <MessageList messages={messages} />
          {isStreaming && streamingContent && (
            <StreamingText content={streamingContent} />
          )}
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
