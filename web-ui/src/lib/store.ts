import { create } from 'zustand'
import type { ChatMessage, AppState, ResultEvent } from './types'

interface ChatStore {
  // Messages
  messages: ChatMessage[]
  isStreaming: boolean
  streamingContent: string

  // App state
  appState: AppState | null

  // Debug
  rawEvents: Array<{ timestamp: number; event: string; data: string }>
  debugPanelOpen: boolean

  // Actions
  addUserMessage: (content: string) => void
  startStreaming: () => void
  appendStreamContent: (text: string) => void
  addAssistantMessage: (msg: ChatMessage) => void
  finishStreaming: (result?: ResultEvent) => void
  setAppState: (state: AppState) => void
  addRawEvent: (event: string, data: string) => void
  toggleDebugPanel: () => void
  clearMessages: () => void
}

export const useChatStore = create<ChatStore>((set, get) => ({
  messages: [],
  isStreaming: false,
  streamingContent: '',
  appState: null,
  rawEvents: [],
  debugPanelOpen: false,

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
    set({ isStreaming: true, streamingContent: '' })
  },

  appendStreamContent: (text: string) => {
    set((state) => ({ streamingContent: state.streamingContent + text }))
  },

  addAssistantMessage: (msg: ChatMessage) => {
    set((state) => ({
      messages: [...state.messages, msg],
      streamingContent: '',
    }))
  },

  finishStreaming: (_result?: ResultEvent) => {
    const { streamingContent, messages } = get()
    // If there's accumulated streaming content not yet committed as a message
    if (streamingContent && !messages.some(m => m.isStreaming)) {
      const msg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: streamingContent,
        timestamp: Date.now(),
      }
      set((state) => ({
        messages: [...state.messages, msg],
        isStreaming: false,
        streamingContent: '',
      }))
    } else {
      set({ isStreaming: false, streamingContent: '' })
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

  clearMessages: () => {
    set({ messages: [], streamingContent: '', rawEvents: [] })
  },
}))
