import type { AgentTreeAction, AppState } from '../app-state.js'

export function reduceAgentTree(state: AppState, action: AgentTreeAction): AppState {
  switch (action.type) {
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
  }
}
