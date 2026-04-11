import React, { createContext, useContext, useReducer, type Dispatch } from 'react'

// --- Types ---

export interface UIMessage {
  id: string
  role: 'user' | 'assistant' | 'system' | 'tool_use' | 'tool_result'
  content: string
  timestamp: number
  // For assistant messages
  costUsd?: number
  // For tool messages
  toolName?: string
  toolInput?: any
  isError?: boolean
  // For system messages
  level?: string
}

export interface Usage {
  inputTokens: number
  outputTokens: number
  costUsd: number
}

export interface PermissionRequest {
  toolUseId: string
  tool: string
  command: string
  options: string[]
}

export interface AppState {
  messages: UIMessage[]
  streamingText: string
  streamingMessageId: string | null
  isStreaming: boolean
  isWaiting: boolean
  model: string
  sessionId: string
  cwd: string
  usage: Usage
  permissionRequest: PermissionRequest | null
  suggestions: string[]
  inputHistory: string[]
  historyIndex: number
  vimEnabled: boolean
  vimMode: string
}

export const initialState: AppState = {
  messages: [],
  streamingText: '',
  streamingMessageId: null,
  isStreaming: false,
  isWaiting: false,
  model: '',
  sessionId: '',
  cwd: '',
  usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
  permissionRequest: null,
  suggestions: [],
  inputHistory: [],
  historyIndex: -1,
  vimEnabled: false,
  vimMode: 'NORMAL',
}

// --- Actions ---

export type AppAction =
  | { type: 'READY'; model: string; sessionId: string; cwd: string }
  | { type: 'ADD_USER_MESSAGE'; id: string; text: string }
  | { type: 'ADD_COMMAND_MESSAGE'; id: string; text: string }
  | { type: 'STREAM_START'; messageId: string }
  | { type: 'STREAM_DELTA'; text: string }
  | { type: 'STREAM_END' }
  | { type: 'ASSISTANT_MESSAGE'; id: string; content: string; costUsd: number }
  | { type: 'TOOL_USE'; id: string; name: string; input: any }
  | { type: 'TOOL_RESULT'; toolUseId: string; output: string; isError: boolean }
  | { type: 'PERMISSION_REQUEST'; request: PermissionRequest }
  | { type: 'PERMISSION_DISMISS' }
  | { type: 'SYSTEM_INFO'; text: string; level: string }
  | { type: 'USAGE_UPDATE'; usage: Usage }
  | { type: 'SUGGESTIONS'; items: string[] }
  | { type: 'ERROR'; message: string }
  | { type: 'PUSH_HISTORY'; text: string }
  | { type: 'SET_HISTORY_INDEX'; index: number }
  | { type: 'SET_VIM_MODE'; mode: string }
  | { type: 'TOGGLE_VIM' }

// --- Reducer ---

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'READY':
      return { ...state, model: action.model, sessionId: action.sessionId, cwd: action.cwd }

    case 'ADD_USER_MESSAGE':
      return {
        ...state,
        isWaiting: true,
        messages: [...state.messages, {
          id: action.id,
          role: 'user',
          content: action.text,
          timestamp: Date.now(),
        }],
      }

    case 'ADD_COMMAND_MESSAGE':
      return {
        ...state,
        // Slash commands don't go through stream cycle, so don't set isWaiting
        messages: [...state.messages, {
          id: action.id,
          role: 'user',
          content: action.text,
          timestamp: Date.now(),
        }],
      }

    case 'STREAM_START':
      return {
        ...state,
        isStreaming: true,
        isWaiting: false,
        streamingText: '',
        streamingMessageId: action.messageId,
      }

    case 'STREAM_DELTA':
      return { ...state, streamingText: state.streamingText + action.text }

    case 'STREAM_END':
      return {
        ...state,
        isStreaming: false,
        isWaiting: false,
        streamingText: '',
        streamingMessageId: null,
      }

    case 'ASSISTANT_MESSAGE': {
      // Replace streaming with final message
      return {
        ...state,
        isStreaming: false,
        isWaiting: false,
        streamingText: '',
        streamingMessageId: null,
        messages: [...state.messages, {
          id: action.id,
          role: 'assistant',
          content: action.content,
          timestamp: Date.now(),
          costUsd: action.costUsd,
        }],
      }
    }

    case 'TOOL_USE':
      return {
        ...state,
        messages: [...state.messages, {
          id: action.id,
          role: 'tool_use',
          content: `Tool: ${action.name}`,
          timestamp: Date.now(),
          toolName: action.name,
          toolInput: action.input,
        }],
      }

    case 'TOOL_RESULT':
      return {
        ...state,
        messages: [...state.messages, {
          id: `result-${action.toolUseId}`,
          role: 'tool_result',
          content: action.output,
          timestamp: Date.now(),
          isError: action.isError,
        }],
      }

    case 'PERMISSION_REQUEST':
      return { ...state, permissionRequest: action.request }

    case 'PERMISSION_DISMISS':
      return { ...state, permissionRequest: null }

    case 'SYSTEM_INFO':
      return {
        ...state,
        messages: [...state.messages, {
          id: `sys-${Date.now()}`,
          role: 'system',
          content: action.text,
          timestamp: Date.now(),
          level: action.level,
        }],
      }

    case 'USAGE_UPDATE':
      return { ...state, usage: action.usage }

    case 'SUGGESTIONS':
      return { ...state, suggestions: action.items }

    case 'ERROR':
      return {
        ...state,
        isStreaming: false,
        isWaiting: false,
        streamingText: '',
        streamingMessageId: null,
        messages: [...state.messages, {
          id: `err-${Date.now()}`,
          role: 'system',
          content: action.message,
          timestamp: Date.now(),
          level: 'error',
        }],
      }

    case 'PUSH_HISTORY':
      return {
        ...state,
        inputHistory: [...state.inputHistory, action.text],
        historyIndex: -1,
      }

    case 'SET_HISTORY_INDEX':
      return { ...state, historyIndex: action.index }

    case 'SET_VIM_MODE':
      return { ...state, vimMode: action.mode }

    case 'TOGGLE_VIM':
      return { ...state, vimEnabled: !state.vimEnabled, vimMode: state.vimEnabled ? '' : 'NORMAL' }

    default:
      return state
  }
}

// --- Context ---

const StateContext = createContext<AppState>(initialState)
const DispatchContext = createContext<Dispatch<AppAction>>(() => {})

export function AppStateProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(appReducer, initialState)
  return (
    <StateContext.Provider value={state}>
      <DispatchContext.Provider value={dispatch}>
        {children}
      </DispatchContext.Provider>
    </StateContext.Provider>
  )
}

export function useAppState(): AppState {
  return useContext(StateContext)
}

export function useAppDispatch(): Dispatch<AppAction> {
  return useContext(DispatchContext)
}
