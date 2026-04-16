import { useCallback } from 'react'
import { useChatStore } from '@/lib/store'
import { sendChatMessage, abortChat } from '@/lib/api'

export function useChat() {
  const messages = useChatStore((s) => s.messages)
  const isStreaming = useChatStore((s) => s.isStreaming)
  const streamingContent = useChatStore((s) => s.streamingContent)

  const sendMessage = useCallback(async (text: string) => {
    if (!text.trim() || isStreaming) return
    await sendChatMessage(text.trim())
  }, [isStreaming])

  const abort = useCallback(async () => {
    await abortChat()
  }, [])

  return {
    messages,
    isStreaming,
    streamingContent,
    sendMessage,
    abort,
  }
}
