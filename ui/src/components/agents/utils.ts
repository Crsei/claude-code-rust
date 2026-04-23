import type { AgentSource } from './types.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/utils.ts`.
 *
 * Upstream routes through `getSettingSourceName` out of the Ink
 * settings store; the Lite port bakes the mapping in directly so the
 * component tree doesn't depend on settings plumbing.
 */

function capitalize(s: string): string {
  return s.length === 0 ? s : s[0]!.toUpperCase() + s.slice(1)
}

const SOURCE_DISPLAY: Record<AgentSource, string> = {
  all: 'Agents',
  'built-in': 'Built-in agents',
  plugin: 'Plugin agents',
  userSettings: 'User',
  projectSettings: 'Project',
  policySettings: 'Policy',
  localSettings: 'Local',
  flagSettings: 'Flag',
}

export function getAgentSourceDisplayName(source: AgentSource): string {
  return SOURCE_DISPLAY[source] ?? capitalize(source)
}
