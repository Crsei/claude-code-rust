import type { AppState, BackgroundAgent, BackgroundAgentAction } from '../app-state.js'

export function reduceBackgroundAgents(state: AppState, action: BackgroundAgentAction): AppState {
  switch (action.type) {
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
  }
}
