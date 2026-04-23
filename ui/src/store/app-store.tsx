import React, { createContext, useContext, useReducer, type Dispatch } from 'react'
import {
  initialState,
  type AppAction,
  type AppState,
} from './app-state.js'
import { reduceAgentSettings } from './reducers/agent-settings.js'
import { reduceAgentTree } from './reducers/agent-tree.js'
import { reduceBackgroundAgents } from './reducers/background-agents.js'
import { reduceCore } from './reducers/core.js'
import { reduceInput } from './reducers/input.js'
import { reduceMcpSettings } from './reducers/mcp-settings.js'
import { reduceSubsystems } from './reducers/subsystems.js'
import { reduceTeams } from './reducers/teams.js'
import { reduceToolActivity } from './reducers/tool-activity.js'

export type {
  AgentStreamState,
  AppAction,
  AppState,
  BackgroundAgent,
  CustomStatusLineState,
  McpSettingsState,
  PendingQuestion,
  PermissionRequest,
  QueuedSubmission,
  ShellProgressState,
  SubsystemState,
  TeamState,
  Usage,
} from './app-state.js'
export { initialState } from './app-state.js'

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'READY':
    case 'REPLACE_MESSAGES':
    case 'ADD_USER_MESSAGE':
    case 'ADD_COMMAND_MESSAGE':
    case 'STREAM_START':
    case 'STREAM_DELTA':
    case 'THINKING_DELTA':
    case 'STREAM_END':
    case 'ASSISTANT_MESSAGE':
    case 'PERMISSION_REQUEST':
    case 'PERMISSION_DISMISS':
    case 'QUESTION_REQUEST':
    case 'QUESTION_DISMISS':
    case 'SYSTEM_INFO':
    case 'USAGE_UPDATE':
    case 'SUGGESTIONS':
    case 'ERROR':
      return reduceCore(state, action)

    case 'TOOL_USE':
    case 'TOOL_RESULT':
    case 'TOOL_PROGRESS':
      return reduceToolActivity(state, action)

    case 'BG_AGENT_STARTED':
    case 'BG_AGENT_COMPLETE':
      return reduceBackgroundAgents(state, action)

    case 'AGENT_TREE_SNAPSHOT':
    case 'AGENT_SPAWNED':
    case 'AGENT_COMPLETED':
    case 'AGENT_ERROR':
    case 'AGENT_ABORTED':
    case 'AGENT_STREAM_DELTA':
    case 'AGENT_THINKING_DELTA':
      return reduceAgentTree(state, action)

    case 'TEAM_MEMBER_JOINED':
    case 'TEAM_MEMBER_LEFT':
    case 'TEAM_MESSAGE_ROUTED':
    case 'TEAM_STATUS_SNAPSHOT':
      return reduceTeams(state, action)

    case 'SUBSYSTEM_STATUS':
    case 'LSP_SERVER_STATE':
    case 'MCP_SERVER_STATE':
    case 'PLUGIN_STATUS':
    case 'SKILLS_LOADED':
    case 'CUSTOM_STATUS_LINE_UPDATE':
    case 'LSP_RECOMMENDATION_REQUEST':
    case 'LSP_RECOMMENDATION_DISMISS':
    case 'LSP_RECOMMENDATION_SETTINGS':
      return reduceSubsystems(state, action)

    case 'AGENT_SETTINGS_OPEN':
    case 'AGENT_SETTINGS_CLOSE':
    case 'AGENT_SETTINGS_LIST':
    case 'AGENT_SETTINGS_CHANGED':
    case 'AGENT_SETTINGS_ERROR':
    case 'AGENT_SETTINGS_CLEAR_NOTICE':
    case 'AGENT_SETTINGS_TOOLS':
    case 'AGENT_SETTINGS_EDITOR_OPENED':
    case 'AGENT_SETTINGS_GENERATE_STARTED':
    case 'AGENT_SETTINGS_GENERATED':
    case 'AGENT_SETTINGS_CLEAR_GENERATED':
      return reduceAgentSettings(state, action)

    case 'MCP_SETTINGS_OPEN':
    case 'MCP_SETTINGS_CLOSE':
    case 'MCP_SETTINGS_CONFIG_LIST':
    case 'MCP_SETTINGS_CONFIG_CHANGED':
    case 'MCP_SETTINGS_CONFIG_ERROR':
    case 'MCP_SETTINGS_SERVER_LIST':
    case 'MCP_SETTINGS_SERVER_STATE':
    case 'MCP_SETTINGS_TOOLS_DISCOVERED':
    case 'MCP_SETTINGS_RESOURCES_DISCOVERED':
    case 'MCP_SETTINGS_CLEAR_NOTICE':
      return reduceMcpSettings(state, action)

    case 'PUSH_HISTORY':
    case 'SET_HISTORY_INDEX':
    case 'SET_EDITOR_MODE':
    case 'SET_VIM_MODE':
    case 'SET_KEYBINDINGS_CONFIG':
    case 'TOGGLE_VIM':
    case 'QUEUE_SUBMISSION':
    case 'DEQUEUE_SUBMISSION':
    case 'SET_VIEW_MODE':
    case 'TOGGLE_VIEW_MODE':
      return reduceInput(state, action)

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
