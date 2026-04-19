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

    case 'SET_EDITOR_MODE':
      return {
        ...state,
        editorMode: action.editorMode,
        vimEnabled: action.editorMode === 'vim',
        vimMode: action.editorMode === 'vim' ? 'NORMAL' : '',
      }

    case 'SET_VIM_MODE':
      return { ...state, vimMode: action.mode }

    case 'SET_KEYBINDINGS_CONFIG':
      return { ...state, keybindingConfig: action.keybindings }

    case 'TOGGLE_VIM':
      return {
        ...state,
        editorMode: state.vimEnabled ? 'normal' : 'vim',
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
