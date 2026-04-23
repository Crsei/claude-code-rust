import type { AppState, McpSettingsAction } from '../app-state.js'
import { upsertBy } from './upsert.js'

/**
 * Reducer for the `/mcp` management dialog state.
 *
 * OpenTUI port of the upstream `MCPSettings` state machine. Mirrors the
 * same events cc-rust already emits (`ConfigList`, `ConfigChanged`,
 * `ConfigError`, `ServerList`, `ServerStateChanged`, `ToolsDiscovered`,
 * `ResourcesDiscovered`) and threads them onto a single slice the
 * dialog subscribes to.
 *
 * Sibling to `reduceAgentSettings`: owns a small notice channel
 * (`lastError`/`lastMessage`) that the dialog renders inline for
 * feedback on upsert / toggle / reconnect actions.
 */
export function reduceMcpSettings(
  state: AppState,
  action: McpSettingsAction,
): AppState {
  switch (action.type) {
    case 'MCP_SETTINGS_OPEN':
      return {
        ...state,
        mcpSettings: { ...state.mcpSettings, open: true },
      }

    case 'MCP_SETTINGS_CLOSE':
      return {
        ...state,
        mcpSettings: { ...state.mcpSettings, open: false },
      }

    case 'MCP_SETTINGS_CONFIG_LIST':
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          entries: action.entries,
          lastUpdated: Date.now(),
        },
      }

    case 'MCP_SETTINGS_CONFIG_CHANGED': {
      const { serverName, entry } = action
      const withoutOld = state.mcpSettings.entries.filter(
        e => !(e.name === serverName && (!entry || sameScope(e.scope, entry.scope))),
      )
      const entries = entry ? [...withoutOld, entry] : withoutOld
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          entries,
          lastError: null,
          lastMessage: entry
            ? `Updated ${serverName} (${scopeLabel(entry.scope)})`
            : `Removed ${serverName}`,
          lastUpdated: Date.now(),
        },
      }
    }

    case 'MCP_SETTINGS_CONFIG_ERROR':
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          lastError: `${action.serverName}: ${action.error}`,
          lastMessage: null,
          lastUpdated: Date.now(),
        },
      }

    case 'MCP_SETTINGS_SERVER_LIST':
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          status: action.servers,
          lastUpdated: Date.now(),
        },
      }

    case 'MCP_SETTINGS_SERVER_STATE':
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          status: upsertBy(
            state.mcpSettings.status,
            'name',
            action.serverName,
            s => ({ ...s, state: action.state, error: action.error }),
          ),
          lastUpdated: Date.now(),
        },
      }

    case 'MCP_SETTINGS_TOOLS_DISCOVERED':
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          toolsByServer: {
            ...state.mcpSettings.toolsByServer,
            [action.serverName]: action.tools,
          },
          status: upsertBy(
            state.mcpSettings.status,
            'name',
            action.serverName,
            s => ({ ...s, tools_count: action.tools.length }),
          ),
          lastUpdated: Date.now(),
        },
      }

    case 'MCP_SETTINGS_RESOURCES_DISCOVERED':
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          resourcesByServer: {
            ...state.mcpSettings.resourcesByServer,
            [action.serverName]: action.resources,
          },
          status: upsertBy(
            state.mcpSettings.status,
            'name',
            action.serverName,
            s => ({ ...s, resources_count: action.resources.length }),
          ),
          lastUpdated: Date.now(),
        },
      }

    case 'MCP_SETTINGS_CLEAR_NOTICE':
      return {
        ...state,
        mcpSettings: {
          ...state.mcpSettings,
          lastError: null,
          lastMessage: null,
        },
      }
  }
}

function sameScope(
  a: { kind: string } & Record<string, unknown>,
  b: { kind: string } & Record<string, unknown>,
): boolean {
  if (a.kind !== b.kind) return false
  if (a.kind === 'plugin' || a.kind === 'ide') {
    return (a as { id?: string }).id === (b as { id?: string }).id
  }
  return true
}

function scopeLabel(scope: { kind: string } & Record<string, unknown>): string {
  switch (scope.kind) {
    case 'user':
      return 'user'
    case 'project':
      return 'project'
    case 'plugin':
      return `plugin:${(scope as { id?: string }).id ?? ''}`
    case 'ide':
      return `ide:${(scope as { id?: string }).id ?? ''}`
    default:
      return scope.kind
  }
}
