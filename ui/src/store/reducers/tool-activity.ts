import type { AppState, ToolActivityAction } from '../app-state.js'

export function reduceToolActivity(state: AppState, action: ToolActivityAction): AppState {
  switch (action.type) {
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
  }
}
