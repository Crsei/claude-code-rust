import type { AppState, SubsystemAction } from '../app-state.js'
import { upsertBy } from './upsert.js'

export function reduceSubsystems(state: AppState, action: SubsystemAction): AppState {
  switch (action.type) {
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

    case 'CUSTOM_STATUS_LINE_UPDATE':
      return {
        ...state,
        customStatusLine: {
          lines: action.lines,
          error: action.error,
          updatedAt: action.updatedAt,
        },
      }

    case 'LSP_RECOMMENDATION_REQUEST':
      return {
        ...state,
        lspRecommendation: {
          ...state.lspRecommendation,
          request: action.payload,
        },
      }

    case 'LSP_RECOMMENDATION_DISMISS':
      return {
        ...state,
        lspRecommendation: {
          ...state.lspRecommendation,
          request: null,
        },
      }

    case 'LSP_RECOMMENDATION_SETTINGS':
      return {
        ...state,
        lspRecommendation: {
          ...state.lspRecommendation,
          settings: action.settings,
        },
      }

    case 'LSP_DIAGNOSTICS_PUBLISHED': {
      const next = { ...state.diagnostics.byUri }
      if (action.diagnostics.length === 0) {
        // Empty list = LSP wants to clear the document; drop the key so
        // aggregate counts in `DiagnosticsDisplay` stay accurate.
        delete next[action.uri]
      } else {
        next[action.uri] = action.diagnostics
      }
      return {
        ...state,
        diagnostics: { byUri: next, lastUpdated: Date.now() },
      }
    }

    case 'LSP_DIAGNOSTICS_CLEAR':
      return {
        ...state,
        diagnostics: { byUri: {}, lastUpdated: Date.now() },
      }

    case 'IDE_CONNECTION_CHANGED':
      return {
        ...state,
        ide: {
          ...state.ide,
          connected: action.connected,
          // Clear the selection when the IDE disconnects; the cached
          // value would misrepresent the live state otherwise.
          selection: action.connected ? state.ide.selection : null,
        },
      }

    case 'IDE_SELECTION_CHANGED':
      return {
        ...state,
        ide: { ...state.ide, selection: action.selection },
      }
  }
}
