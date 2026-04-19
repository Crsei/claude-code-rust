import type { AppState, InputAction } from '../app-state.js'

export function reduceInput(state: AppState, action: InputAction): AppState {
  switch (action.type) {
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
  }
}
