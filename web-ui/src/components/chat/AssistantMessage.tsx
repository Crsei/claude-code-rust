import type { ChatMessage } from '@/lib/types'

interface AssistantMessageProps {
  message: ChatMessage
}

export function AssistantMessage({ message }: AssistantMessageProps) {
  return (
    <div className="flex justify-start">
      <div className="max-w-[85%] space-y-2">
        <div className="rounded-2xl rounded-bl-md bg-secondary px-4 py-2.5 text-sm text-secondary-foreground">
          <div className="prose prose-sm prose-invert max-w-none whitespace-pre-wrap">
            {message.content}
          </div>
        </div>
        {message.usage && (
          <div className="flex gap-3 px-1 text-[10px] text-muted-foreground">
            <span>in: {message.usage.input_tokens}</span>
            <span>out: {message.usage.output_tokens}</span>
            {message.costUsd != null && <span>${message.costUsd.toFixed(4)}</span>}
          </div>
        )}
      </div>
    </div>
  )
}
