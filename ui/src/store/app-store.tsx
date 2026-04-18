import React, { createContext, useContext, useReducer, type Dispatch } from 'react'
import type {
  AgentNode,
  FrontendContentBlock,
  LspServerInfo,
  McpServerStatusInfo,
  PluginInfo,
  SkillInfo,
  TeamMemberInfo,
} from '../ipc/protocol.js'
import type { ViewMode } from '../keybindings.js'
import type { RawMessage } from './message-model.js'

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

export interface BackgroundAgent {
  agentId: string
  description: string
  startedAt: number
  completedAt?: number
  resultPreview?: string
  hadError?: boolean
  durationMs?: number
}

export interface PendingQuestion {
  id: string
  text: string
}

export interface QueuedSubmission {
  id: string
  kind: 'prompt'
  text: string
  queuedAt: number
}

// ---------------------------------------------------------------------------
// Agent tree state
// ---------------------------------------------------------------------------

export interface AgentStreamState {
  text: string
  thinking: string
}

// ---------------------------------------------------------------------------
// Team state
// ---------------------------------------------------------------------------

export interface TeamState {
  name: string
  members: TeamMemberInfo[]
  pendingMessages: number
  recentMessages: Array<{ from: string; to: string; summary: string; timestamp: number }>
}

// ---------------------------------------------------------------------------
// Subsystem state
// ---------------------------------------------------------------------------

export interface SubsystemState {
  lsp: LspServerInfo[]
  mcp: McpServerStatusInfo[]
  plugins: PluginInfo[]
  skills: SkillInfo[]
  lastUpdated: number
}

export interface AppState {
  messages: RawMessage[]
  streamingText: string
  streamingThinking: string
  streamingMessageId: string | null
  isStreaming: boolean
  isWaiting: boolean
  model: string
  sessionId: string
  cwd: string
  usage: Usage
  permissionRequest: PermissionRequest | null
  pendingQuestion: PendingQuestion | null
  suggestions: string[]
  inputHistory: string[]
  historyIndex: number
  vimEnabled: boolean
  vimMode: string
  backgroundAgents: BackgroundAgent[]
  queuedSubmissions: QueuedSubmission[]
  viewMode: ViewMode
  // Agent tree
  agentTree: AgentNode[]
  agentStreams: Record<string, AgentStreamState>
  // Teams
  teams: Record<string, TeamState>
  // Subsystems
  subsystems: SubsystemState
}

export const initialState: AppState = {
  messages: [],
  streamingText: '',
  streamingThinking: '',
  streamingMessageId: null,
  isStreaming: false,
  isWaiting: false,
  model: '',
  sessionId: '',
  cwd: '',
  usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
  permissionRequest: null,
  pendingQuestion: null,
  suggestions: [],
  inputHistory: [],
  historyIndex: -1,
  vimEnabled: false,
  vimMode: 'NORMAL',
  backgroundAgents: [],
  queuedSubmissions: [],
  viewMode: 'prompt',
  agentTree: [],
  agentStreams: {},
  teams: {},
  subsystems: { lsp: [], mcp: [], plugins: [], skills: [], lastUpdated: 0 },
}

export type AppAction =
  | { type: 'READY'; model: string; sessionId: string; cwd: string }
  | { type: 'REPLACE_MESSAGES'; messages: RawMessage[] }
  | { type: 'ADD_USER_MESSAGE'; id: string; text: string }
  | { type: 'ADD_COMMAND_MESSAGE'; id: string; text: string }
  | { type: 'STREAM_START'; messageId: string }
  | { type: 'STREAM_DELTA'; text: string }
  | { type: 'THINKING_DELTA'; thinking: string }
  | { type: 'STREAM_END' }
  | {
      type: 'ASSISTANT_MESSAGE'
      id: string
      content: string
      contentBlocks?: FrontendContentBlock[]
      costUsd: number
      thinking?: string
    }
  | { type: 'TOOL_USE'; id: string; name: string; input: any }
  | { type: 'TOOL_RESULT'; toolUseId: string; output: string; isError: boolean }
  | { type: 'PERMISSION_REQUEST'; request: PermissionRequest }
  | { type: 'PERMISSION_DISMISS' }
  | { type: 'QUESTION_REQUEST'; question: PendingQuestion }
  | { type: 'QUESTION_DISMISS' }
  | { type: 'SYSTEM_INFO'; text: string; level: string }
  | { type: 'USAGE_UPDATE'; usage: Usage }
  | { type: 'SUGGESTIONS'; items: string[] }
  | { type: 'ERROR'; message: string }
  | { type: 'BG_AGENT_STARTED'; agentId: string; description: string }
  | {
      type: 'BG_AGENT_COMPLETE'
      agentId: string
      description: string
      resultPreview: string
      hadError: boolean
      durationMs: number
    }
  | { type: 'PUSH_HISTORY'; text: string }
  | { type: 'SET_HISTORY_INDEX'; index: number }
  | { type: 'SET_VIM_MODE'; mode: string }
  | { type: 'TOGGLE_VIM' }
  | { type: 'QUEUE_SUBMISSION'; submission: QueuedSubmission }
  | { type: 'DEQUEUE_SUBMISSION' }
  | { type: 'SET_VIEW_MODE'; viewMode: ViewMode }
  | { type: 'TOGGLE_VIEW_MODE' }
  // Agent tree
  | { type: 'AGENT_TREE_SNAPSHOT'; roots: AgentNode[] }
  | { type: 'AGENT_SPAWNED'; agentId: string; description: string; parentAgentId?: string; agentType?: string; model?: string; isBackground: boolean; depth: number }
  | { type: 'AGENT_COMPLETED'; agentId: string; resultPreview: string; hadError: boolean; durationMs: number }
  | { type: 'AGENT_ERROR'; agentId: string; error: string; durationMs: number }
  | { type: 'AGENT_ABORTED'; agentId: string }
  | { type: 'AGENT_STREAM_DELTA'; agentId: string; text: string }
  | { type: 'AGENT_THINKING_DELTA'; agentId: string; thinking: string }
  // Teams
  | { type: 'TEAM_MEMBER_JOINED'; teamName: string; agentId: string; agentName: string; role: string }
  | { type: 'TEAM_MEMBER_LEFT'; teamName: string; agentId: string }
  | { type: 'TEAM_MESSAGE_ROUTED'; teamName: string; from: string; to: string; summary: string; timestamp: number }
  | { type: 'TEAM_STATUS_SNAPSHOT'; teamName: string; members: TeamMemberInfo[]; pendingMessages: number }
  // Subsystems
  | { type: 'SUBSYSTEM_STATUS'; lsp: LspServerInfo[]; mcp: McpServerStatusInfo[]; plugins: PluginInfo[]; skills: SkillInfo[] }
  | { type: 'LSP_SERVER_STATE'; languageId: string; state: string; error?: string }
  | { type: 'MCP_SERVER_STATE'; serverName: string; state: string; error?: string }
  | { type: 'PLUGIN_STATUS'; pluginId: string; name: string; status: string; error?: string }
  | { type: 'SKILLS_LOADED'; count: number }

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'READY':
      return { ...state, model: action.model, sessionId: action.sessionId, cwd: action.cwd }

    case 'REPLACE_MESSAGES':
      return {
        ...state,
        isStreaming: false,
        isWaiting: false,
        streamingText: '',
        streamingThinking: '',
        streamingMessageId: null,
        messages: action.messages,
      }

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
        streamingThinking: '',
        streamingMessageId: action.messageId,
      }

    case 'STREAM_DELTA':
      return { ...state, streamingText: state.streamingText + action.text }

    case 'THINKING_DELTA':
      return { ...state, streamingThinking: state.streamingThinking + action.thinking }

    case 'STREAM_END':
      return {
        ...state,
        isStreaming: false,
        isWaiting: false,
        streamingText: '',
        streamingThinking: '',
        streamingMessageId: null,
      }

    case 'ASSISTANT_MESSAGE':
      return {
        ...state,
        isStreaming: false,
        isWaiting: false,
        streamingText: '',
        streamingThinking: '',
        streamingMessageId: null,
        messages: [...state.messages, {
          id: action.id,
          role: 'assistant',
          content: action.content,
          timestamp: Date.now(),
          contentBlocks: action.contentBlocks,
          costUsd: action.costUsd,
          thinking: action.thinking,
        }],
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
          toolUseId: action.id,
        }],
      }

    case 'TOOL_RESULT':
      return {
        ...state,
        messages: [...state.messages, {
          id: `result-${action.toolUseId}-${Date.now()}`,
          role: 'tool_result',
          content: action.output,
          timestamp: Date.now(),
          toolUseId: action.toolUseId,
          isError: action.isError,
        }],
      }

    case 'PERMISSION_REQUEST':
      return { ...state, permissionRequest: action.request }

    case 'PERMISSION_DISMISS':
      return { ...state, permissionRequest: null }

    case 'QUESTION_REQUEST':
      return {
        ...state,
        pendingQuestion: action.question,
        messages: [...state.messages, {
          id: `question-${action.question.id}`,
          role: 'system',
          content: action.question.text,
          timestamp: Date.now(),
          level: 'question',
        }],
      }

    case 'QUESTION_DISMISS':
      return { ...state, pendingQuestion: null }

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
        streamingThinking: '',
        streamingMessageId: null,
        messages: [...state.messages, {
          id: `err-${Date.now()}`,
          role: 'system',
          content: action.message,
          timestamp: Date.now(),
          level: 'error',
        }],
      }

    case 'BG_AGENT_STARTED': {
      const agent: BackgroundAgent = {
        agentId: action.agentId,
        description: action.description,
        startedAt: Date.now(),
      }
      return {
        ...state,
        backgroundAgents: [...state.backgroundAgents, agent],
        messages: [...state.messages, {
          id: `bg-start-${action.agentId}`,
          role: 'system',
          content: `Background agent started: ${action.description}`,
          timestamp: Date.now(),
          level: 'info',
        }],
      }
    }

    case 'BG_AGENT_COMPLETE': {
      const durationSec = (action.durationMs / 1000).toFixed(1)
      const statusLabel = action.hadError ? 'FAILED' : 'DONE'
      return {
        ...state,
        backgroundAgents: state.backgroundAgents.map(agent =>
          agent.agentId === action.agentId
            ? {
                ...agent,
                completedAt: Date.now(),
                resultPreview: action.resultPreview,
                hadError: action.hadError,
                durationMs: action.durationMs,
              }
            : agent,
        ),
        messages: [...state.messages, {
          id: `bg-done-${action.agentId}`,
          role: 'system',
          content: `${statusLabel} background agent "${action.description}" completed in ${durationSec}s\n\n${action.resultPreview}`,
          timestamp: Date.now(),
          level: action.hadError ? 'error' : 'info',
        }],
      }
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
      return {
        ...state,
        vimEnabled: !state.vimEnabled,
        vimMode: state.vimEnabled ? '' : 'NORMAL',
      }

    case 'QUEUE_SUBMISSION':
      return {
        ...state,
        queuedSubmissions: [...state.queuedSubmissions, action.submission],
      }

    case 'DEQUEUE_SUBMISSION':
      return {
        ...state,
        queuedSubmissions: state.queuedSubmissions.slice(1),
      }

    case 'SET_VIEW_MODE':
      return { ...state, viewMode: action.viewMode }

    case 'TOGGLE_VIEW_MODE':
      return {
        ...state,
        viewMode: state.viewMode === 'prompt' ? 'transcript' : 'prompt',
      }

    // ── Agent tree ──────────────────────────────────────────────
    case 'AGENT_TREE_SNAPSHOT':
      return { ...state, agentTree: action.roots }

    case 'AGENT_SPAWNED':
      return {
        ...state,
        agentStreams: { ...state.agentStreams, [action.agentId]: { text: '', thinking: '' } },
        messages: [...state.messages, {
          id: `agent-spawn-${action.agentId}`,
          role: 'system',
          content: `Agent spawned: ${action.description}${action.isBackground ? ' (background)' : ''}`,
          timestamp: Date.now(),
          level: 'info',
        }],
      }

    case 'AGENT_COMPLETED': {
      const { [action.agentId]: _removed, ...remainingStreams } = state.agentStreams
      const durationSec = (action.durationMs / 1000).toFixed(1)
      return {
        ...state,
        agentStreams: remainingStreams,
        messages: [...state.messages, {
          id: `agent-done-${action.agentId}`,
          role: 'system',
          content: `Agent ${action.hadError ? 'FAILED' : 'completed'} (${durationSec}s): ${action.resultPreview}`,
          timestamp: Date.now(),
          level: action.hadError ? 'error' : 'info',
        }],
      }
    }

    case 'AGENT_ERROR': {
      const { [action.agentId]: _removed, ...remainingStreams } = state.agentStreams
      return {
        ...state,
        agentStreams: remainingStreams,
        messages: [...state.messages, {
          id: `agent-err-${action.agentId}`,
          role: 'system',
          content: `Agent error (${(action.durationMs / 1000).toFixed(1)}s): ${action.error}`,
          timestamp: Date.now(),
          level: 'error',
        }],
      }
    }

    case 'AGENT_ABORTED': {
      const { [action.agentId]: _removed, ...remainingStreams } = state.agentStreams
      return {
        ...state,
        agentStreams: remainingStreams,
        messages: [...state.messages, {
          id: `agent-abort-${action.agentId}`,
          role: 'system',
          content: `Agent aborted: ${action.agentId}`,
          timestamp: Date.now(),
          level: 'warning',
        }],
      }
    }

    case 'AGENT_STREAM_DELTA': {
      const prev = state.agentStreams[action.agentId] ?? { text: '', thinking: '' }
      return {
        ...state,
        agentStreams: { ...state.agentStreams, [action.agentId]: { ...prev, text: prev.text + action.text } },
      }
    }

    case 'AGENT_THINKING_DELTA': {
      const prev = state.agentStreams[action.agentId] ?? { text: '', thinking: '' }
      return {
        ...state,
        agentStreams: { ...state.agentStreams, [action.agentId]: { ...prev, thinking: prev.thinking + action.thinking } },
      }
    }

    // ── Teams ───────────────────────────────────────────────────
    case 'TEAM_MEMBER_JOINED': {
      const team = state.teams[action.teamName] ?? { name: action.teamName, members: [], pendingMessages: 0, recentMessages: [] }
      const newMember: TeamMemberInfo = { agent_id: action.agentId, agent_name: action.agentName, role: action.role, is_active: true, unread_messages: 0 }
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, members: [...team.members, newMember] } },
      }
    }

    case 'TEAM_MEMBER_LEFT': {
      const team = state.teams[action.teamName]
      if (!team) return state
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, members: team.members.filter(m => m.agent_id !== action.agentId) } },
      }
    }

    case 'TEAM_MESSAGE_ROUTED': {
      const team = state.teams[action.teamName] ?? { name: action.teamName, members: [], pendingMessages: 0, recentMessages: [] }
      const msg = { from: action.from, to: action.to, summary: action.summary, timestamp: action.timestamp }
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, recentMessages: [...team.recentMessages.slice(-19), msg] } },
      }
    }

    case 'TEAM_STATUS_SNAPSHOT': {
      const team = state.teams[action.teamName] ?? { name: action.teamName, members: [], pendingMessages: 0, recentMessages: [] }
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, members: action.members, pendingMessages: action.pendingMessages } },
      }
    }

    // ── Subsystems ──────────────────────────────────────────────
    case 'SUBSYSTEM_STATUS':
      return {
        ...state,
        subsystems: { lsp: action.lsp, mcp: action.mcp, plugins: action.plugins, skills: action.skills, lastUpdated: Date.now() },
      }

    case 'LSP_SERVER_STATE':
      return {
        ...state,
        subsystems: {
          ...state.subsystems,
          lsp: upsertBy(state.subsystems.lsp, 'language_id', action.languageId, s => ({ ...s, state: action.state, error: action.error })),
          lastUpdated: Date.now(),
        },
      }

    case 'MCP_SERVER_STATE':
      return {
        ...state,
        subsystems: {
          ...state.subsystems,
          mcp: upsertBy(state.subsystems.mcp, 'name', action.serverName, s => ({ ...s, state: action.state, error: action.error })),
          lastUpdated: Date.now(),
        },
      }

    case 'PLUGIN_STATUS':
      return {
        ...state,
        subsystems: {
          ...state.subsystems,
          plugins: upsertBy(state.subsystems.plugins, 'id', action.pluginId, s => ({ ...s, name: action.name, status: action.status, error: action.error })),
          lastUpdated: Date.now(),
        },
      }

    case 'SKILLS_LOADED':
      return state // informational only — real data arrives via SUBSYSTEM_STATUS

    default:
      return state
  }
}

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Update an item in an array by key, or append a stub if missing. */
function upsertBy<T extends Record<string, any>>(
  arr: T[],
  key: keyof T,
  value: any,
  updater: (existing: T) => T,
): T[] {
  const idx = arr.findIndex(item => item[key] === value)
  if (idx >= 0) {
    const copy = [...arr]
    copy[idx] = updater(copy[idx]!)
    return copy
  }
  return [...arr, updater({ [key]: value } as T)]
}
