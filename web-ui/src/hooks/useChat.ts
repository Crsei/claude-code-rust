import { useCallback } from 'react'
import { useChatStore } from '@/lib/store'
import { sendChatMessage, abortChat } from '@/lib/api'

export function useChat() {
  const messages = useChatStore((s) => s.messages)
  const isStreaming = useChatStore((s) => s.isStreaming)
  const streamingContent = useChatStore((s) => s.streamingContent)
  const streamingBlocks = useChatStore((s) => s.streamingBlocks)

  const sendMessage = useCallback((text: string) => {
    const trimmed = text.trim()
    if (!trimmed || useChatStore.getState().isStreaming) return
    sendChatMessage(trimmed)
  }, [])

  const abort = useCallback(() => {
    abortChat()
  }, [])

  return {
    messages,
    isStreaming,
    streamingContent,
    streamingBlocks,
    sendMessage,
    abort,
  }
}
