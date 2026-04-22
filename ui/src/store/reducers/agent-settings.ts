import type { AppState, AgentSettingsAction } from '../app-state.js'

/**
 * Reducer for the `/agents` settings dialog. Owns both transient UI state
 * (open flag, last error/message notice) and the persisted entries list that
 * the backend sends via `AgentSettingsEvent`.
 */
export function reduceAgentSettings(state: AppState, action: AgentSettingsAction): AppState {
  switch (action.type) {
    case 'AGENT_SETTINGS_OPEN':
      return {
        ...state,
        agentSettings: {
          ...state.agentSettings,
          open: true,
          lastError: null,
          lastMessage: null,
        },
      }

    case 'AGENT_SETTINGS_CLOSE':
      return {
        ...state,
        agentSettings: {
          ...state.agentSettings,
          open: false,
          lastError: null,
          lastMessage: null,
        },
      }

    case 'AGENT_SETTINGS_LIST':
      return {
        ...state,
        agentSettings: {
          ...state.agentSettings,
          entries: action.entries,
          lastUpdated: Date.now(),
          lastError: null,
        },
      }

    case 'AGENT_SETTINGS_CHANGED': {
      const existing = state.agentSettings.entries
      const next = action.entry
        ? upsertEntry(existing, action.entry)
        : existing.filter(
            e =>
              !(e.name === action.name && entrySourceMatches(e, action.entry?.source ?? null)),
          )
      const verb = action.entry ? 'Saved' : 'Deleted'
      return {
        ...state,
        agentSettings: {
          ...state.agentSettings,
          entries: next,
          lastUpdated: Date.now(),
          lastError: null,
          lastMessage: `${verb} agent: ${action.name}`,
        },
      }
    }

    case 'AGENT_SETTINGS_ERROR':
      return {
        ...state,
        agentSettings: {
          ...state.agentSettings,
          lastError: `${action.name}: ${action.error}`,
          lastMessage: null,
        },
      }

    case 'AGENT_SETTINGS_CLEAR_NOTICE':
      return {
        ...state,
        agentSettings: {
          ...state.agentSettings,
          lastError: null,
          lastMessage: null,
        },
      }
  }
}

function upsertEntry(
  list: AppState['agentSettings']['entries'],
  entry: AppState['agentSettings']['entries'][number],
): AppState['agentSettings']['entries'] {
  const idx = list.findIndex(
    e => e.name === entry.name && entrySourceMatches(e, entry.source),
  )
  if (idx < 0) return [...list, entry]
  const next = [...list]
  next[idx] = entry
  return next
}

/**
 * Entries are keyed on `(name, source)` — two sources can contribute an
 * agent with the same name (e.g. a project override of a user agent), so we
 * compare the full source tag plus any source-specific id.
 */
function entrySourceMatches(
  a: AppState['agentSettings']['entries'][number],
  b: AppState['agentSettings']['entries'][number]['source'] | null,
): boolean {
  if (b === null) return true
  if (a.source.kind !== b.kind) return false
  if (a.source.kind === 'plugin' && b.kind === 'plugin') {
    return a.source.id === b.id
  }
  return true
}
