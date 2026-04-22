/**
 * Barrel for the operational-panel card primitives. Each card renders
 * a single entry of the active protocol's subsystem snapshot using a
 * shared `state-colors.ts` palette. Pure data helpers (`summarizeTeam`
 * / `summarizeTeams`) are unit-tested under `__tests__/`.
 *
 * Upstream panels that cannot currently be supported by the Lite
 * protocol are documented in `docs/ui-panels-deferred.md` rather than
 * half-implemented here.
 */
export { LspServerCard } from './LspServerCard.js'
export { McpServerCard } from './McpServerCard.js'
export { PluginRow } from './PluginRow.js'
export { TeamMemberCard } from './TeamMemberCard.js'
export {
  summarizeTeam,
  summarizeTeams,
  type TeamRollupSummary,
  type TeamSummary,
} from './team-summary.js'
export { isHealthyState, stateColor } from './state-colors.js'
