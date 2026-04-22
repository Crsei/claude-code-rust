/**
 * Barrel for the Lite-native statusline (Issue 07).
 *
 * Consumers import `StatusLine` — an orchestrator that always renders
 * the built-in statusline derived from the store and optionally adds
 * a second row with the user-configured custom statusline forwarded
 * via the `status_line_update` IPC event.
 *
 * Pure helpers (`shouldRenderCustomStatusLine`, counting helpers) are
 * exported so tests can cover the derivation without mounting.
 */
export { BuiltinStatusLine } from './BuiltinStatusLine.js'
export { CustomStatusLine } from './CustomStatusLine.js'
export { StatusLine } from './StatusLine.js'
export {
  countActiveTeams,
  countConnectedMcp,
  countRunningAgents,
  countRunningLsp,
  cwdShortName,
  shouldRenderCustomStatusLine,
  statusLineError,
} from './status-line-state.js'
