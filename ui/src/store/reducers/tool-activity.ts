import type { AppState, ShellProgressState, ToolActivityAction } from '../app-state.js'

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

    case 'TOOL_RESULT': {
      const nextMessages = [...state.messages, {
        id: `result-${action.toolUseId}-${Date.now()}`,
        role: 'tool_result' as const,
        content: action.output,
        timestamp: Date.now(),
        toolUseId: action.toolUseId,
        isError: action.isError,
      }]

      const existingProgress = state.shellProgress[action.toolUseId]
      if (!existingProgress) {
        return { ...state, messages: nextMessages }
      }
      // Mark the progress entry as completed so the UI can swap the
      // live tail for the final result summary without losing the
      // "Running → Done" transition.
      return {
        ...state,
        messages: nextMessages,
        shellProgress: {
          ...state.shellProgress,
          [action.toolUseId]: { ...existingProgress, completed: true, updatedAt: Date.now() },
        },
      }
    }

    case 'TOOL_PROGRESS': {
      const existing = state.shellProgress[action.toolUseId]
      const fullOutput = existing ? mergeFullOutput(existing.fullOutput, action.output) : action.output
      const next: ShellProgressState = {
        toolUseId: action.toolUseId,
        tool: action.tool,
        output: action.output,
        fullOutput,
        elapsedSeconds: action.elapsedSeconds,
        totalLines: action.totalLines,
        totalBytes: action.totalBytes,
        timeoutMs: action.timeoutMs,
        completed: false,
        updatedAt: Date.now(),
      }
      return {
        ...state,
        shellProgress: { ...state.shellProgress, [action.toolUseId]: next },
      }
    }
  }
}

/**
 * The backend's `output` is already a tail-capped snapshot. Each tick
 * replaces the snapshot rather than appending, but we keep the longest
 * observed version as `fullOutput` so verbose mode retains earlier
 * content when the backend starts truncating newer ticks.
 */
function mergeFullOutput(previous: string, next: string): string {
  return next.length >= previous.length ? next : previous
}
