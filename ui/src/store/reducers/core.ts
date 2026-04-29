import type { AppState, CoreAction } from '../app-state.js'
import { normalizeEditorMode } from '../../keybindings.js'

export function reduceCore(state: AppState, action: CoreAction): AppState {
  switch (action.type) {
    case 'READY': {
      const editorMode = action.editorMode === undefined
        ? state.editorMode
        : normalizeEditorMode(action.editorMode)
      return {
        ...state,
        model: action.model,
        sessionId: action.sessionId,
        cwd: action.cwd,
        permissionMode: action.permissionMode ?? state.permissionMode,
        planWorkflow: action.planWorkflow === undefined ? state.planWorkflow : action.planWorkflow,
        editorMode,
        vimEnabled: editorMode === 'vim',
        vimMode: editorMode === 'vim' ? 'NORMAL' : '',
        viewMode: action.viewMode === undefined ? state.viewMode : action.viewMode ?? 'prompt',
        keybindingConfig: action.keybindings === undefined ? state.keybindingConfig : action.keybindings,
      }
    }

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

    case 'PLAN_WORKFLOW_EVENT':
      return {
        ...state,
        planWorkflow: action.record,
        permissionMode: action.record.status === 'draft' ? 'plan' : state.permissionMode,
        messages: [...state.messages, {
          id: `plan-${Date.now()}`,
          role: 'system',
          content: action.summary,
          timestamp: Date.now(),
          level: action.event.includes('rejected') ? 'warning' : 'info',
        }],
      }

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
  }
}
