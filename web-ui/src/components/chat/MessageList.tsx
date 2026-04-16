import type { ChatMessage } from '@/lib/types'
import { UserMessage } from './UserMessage'
import { AssistantMessage } from './AssistantMessage'

interface MessageListProps {
  messages: ChatMessage[]
}

export function MessageList({ messages }: MessageListProps) {
  return (
    <>
      {messages.map((msg) => {
        switch (msg.role) {
          case 'user':
            return <UserMessage key={msg.id} message={msg} />
          case 'assistant':
            return <AssistantMessage key={msg.id} message={msg} />
          case 'system':
            return (
              <div
                key={msg.id}
                className="rounded-md border border-border bg-muted/50 px-4 py-2 text-sm text-muted-foreground"
              >
                {msg.content}
              </div>
            )
          default:
            return null
        }
      })}
    </>
  )
}
