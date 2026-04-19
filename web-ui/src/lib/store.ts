import { create } from 'zustand'
import type { ChatMessage, AppState, ContentBlock, ResultEvent, StreamingBlock } from './types'

interface ChatStore {
  // Messages
  messages: ChatMessage[]
  isStreaming: boolean
  streamingContent: string

  // Multi-block streaming state
  streamingBlocks: StreamingBlock[]

  // App state
  appState: AppState | null

  // Debug
  rawEvents: Array<{ timestamp: number; event: string; data: string }>
  debugPanelOpen: boolean
  debugTab: 'events' | 'messages' | 'timeline'

  // Phase 4: Result tracking
  lastResult: ResultEvent | null

  // Actions
  addUserMessage: (content: string) => void
  startStreaming: () => void
  appendStreamContent: (text: string) => void
  addAssistantMessage: (msg: ChatMessage) => void
  /**
   * Attach tool_result content blocks to the most recent assistant message.
   *
   * The Rust engine emits tool results as a separate `user_replay` SSE event
   * (they legally belong to a synthetic user message that replays them back
   * to the model). Here we stitch them onto the preceding assistant message
   * so `AssistantMessage.tsx` can pair each tool_use with its result — which
   * is what makes screenshots, page text, and console logs actually show up
   * under the matching tool card.
   */
  appendToolResultsToLastAssistant: (blocks: ContentBlock[]) => void
  finishStreaming: (result?: ResultEvent) => void
  setAppState: (state: AppState) => void
  addRawEvent: (event: string, data: string) => void
  toggleDebugPanel: () => void
  setDebugTab: (tab: 'events' | 'messages' | 'timeline') => void
  clearMessages: () => void

  // Phase 2: Multi-block streaming actions
  startStreamingBlock: (index: number, block: StreamingBlock) => void
  appendToStreamingBlock: (index: number, text: string) => void
  appendToolInputDelta: (index: number, jsonChunk: string) => void
  finishStreamingBlock: (index: number) => void
  clearStreamingBlocks: () => void
}

export const useChatStore = create<ChatStore>((set, get) => ({
  messages: [],
  isStreaming: false,
  streamingContent: '',
  streamingBlocks: [],
  appState: null,
  rawEvents: [],
  debugPanelOpen: false,
  debugTab: 'events' as const,
  lastResult: null,

  addUserMessage: (content: string) => {
    const msg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content,
      timestamp: Date.now(),
    }
    set((state) => ({ messages: [...state.messages, msg] }))
  },

  startStreaming: () => {
    set({ isStreaming: true, streamingContent: '', streamingBlocks: [] })
  },

  appendStreamContent: (text: string) => {
    set((state) => ({ streamingContent: state.streamingContent + text }))
  },

  addAssistantMessage: (msg: ChatMessage) => {
    set((state) => ({
      messages: [...state.messages, msg],
      streamingContent: '',
      streamingBlocks: [],
    }))
  },

  appendToolResultsToLastAssistant: (blocks: ContentBlock[]) => {
    if (blocks.length === 0) return
    set((state) => {
      // Walk backwards to find the most recent assistant message. System
      // messages (api_retry, tool_use_summary) don't own tool calls.
      for (let i = state.messages.length - 1; i >= 0; i--) {
        const m = state.messages[i]
        if (m.role !== 'assistant') continue
        const existing = m.contentBlocks ?? []
        // Skip results that are already attached (idempotent against
        // reconnects / duplicate replay events).
        const seen = new Set(
          existing
            .filter(b => b.type === 'tool_result' && b.tool_use_id)
            .map(b => b.tool_use_id as string),
        )
        const fresh = blocks.filter(
          b => b.type === 'tool_result' && b.tool_use_id && !seen.has(b.tool_use_id),
        )
        if (fresh.length === 0) return state
        const updated: ChatMessage = {
          ...m,
          contentBlocks: [...existing, ...fresh],
        }
        const next = state.messages.slice()
        next[i] = updated
        return { messages: next }
      }
      return state
    })
  },

  finishStreaming: (result?: ResultEvent) => {
    if (result) {
      set({ lastResult: result })
    }
    const { streamingContent, streamingBlocks, messages } = get()
    // If there's accumulated streaming content not yet committed as a message
    if ((streamingContent || streamingBlocks.length > 0) && !messages.some(m => m.isStreaming)) {
      // Build content blocks from streaming blocks
      const contentBlocks = streamingBlocks
        .filter(b => b.content || b.toolName)
        .map(b => {
          if (b.type === 'text') {
            return { type: 'text' as const, text: b.content }
          } else if (b.type === 'tool_use') {
            let parsedInput: Record<string, unknown> = {}
            try { parsedInput = JSON.parse(b.toolInput || '{}') } catch { /* ignore */ }
            return {
              type: 'tool_use' as const,
              id: b.toolId,
              name: b.toolName,
              input: parsedInput,
            }
          } else if (b.type === 'thinking') {
            return { type: 'thinking' as const, thinking: b.content }
          }
          return { type: 'text' as const, text: b.content }
        })

      const textContent = streamingBlocks
        .filter(b => b.type === 'text')
        .map(b => b.content)
        .join('') || streamingContent

      const msg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: textContent,
        timestamp: Date.now(),
        contentBlocks: contentBlocks.length > 0 ? contentBlocks : undefined,
        toolCalls: contentBlocks.filter(b => b.type === 'tool_use'),
      }
      set((state) => ({
        messages: [...state.messages, msg],
        isStreaming: false,
        streamingContent: '',
        streamingBlocks: [],
      }))
    } else {
      set({ isStreaming: false, streamingContent: '', streamingBlocks: [] })
    }
  },

  setAppState: (appState: AppState) => set({ appState }),

  addRawEvent: (event: string, data: string) => {
    set((state) => ({
      rawEvents: [
        ...state.rawEvents.slice(-499), // keep last 500
        { timestamp: Date.now(), event, data },
      ],
    }))
  },

  toggleDebugPanel: () => {
    set((state) => ({ debugPanelOpen: !state.debugPanelOpen }))
  },

  setDebugTab: (tab: 'events' | 'messages' | 'timeline') => {
    set({ debugTab: tab })
  },

  clearMessages: () => {
    set({ messages: [], streamingContent: '', streamingBlocks: [], rawEvents: [] })
  },

  // Phase 2: Multi-block streaming

  startStreamingBlock: (index: number, block: StreamingBlock) => {
    set((state) => {
      const blocks = [...state.streamingBlocks]
      blocks[index] = block
      return { streamingBlocks: blocks }
    })
  },

  appendToStreamingBlock: (index: number, text: string) => {
    set((state) => {
      const blocks = [...state.streamingBlocks]
      if (blocks[index]) {
        blocks[index] = { ...blocks[index], content: blocks[index].content + text }
      }
      return { streamingBlocks: blocks }
    })
  },

  appendToolInputDelta: (index: number, jsonChunk: string) => {
    set((state) => {
      const blocks = [...state.streamingBlocks]
      if (blocks[index]) {
        blocks[index] = {
          ...blocks[index],
          toolInput: (blocks[index].toolInput || '') + jsonChunk,
        }
      }
      return { streamingBlocks: blocks }
    })
  },

  finishStreamingBlock: (index: number) => {
    set((state) => {
      const blocks = [...state.streamingBlocks]
      if (blocks[index]) {
        blocks[index] = { ...blocks[index], done: true }
      }
      return { streamingBlocks: blocks }
    })
  },

  clearStreamingBlocks: () => {
    set({ streamingBlocks: [] })
  },
}))
