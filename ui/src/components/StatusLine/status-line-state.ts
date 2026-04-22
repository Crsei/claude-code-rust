import type { AgentNode } from '../../ipc/protocol.js'
import type {
  CustomStatusLineState,
  SubsystemState,
  TeamState,
} from '../../store/app-store.js'

/**
 * Pure derivation helpers that back the `BuiltinStatusLine`. Kept out
 * of the component so the count math + custom-line gating can be
 * unit-tested without mounting.
 *
 * Lite-native sibling of the sample tree's `StatusLine.tsx` +
 * `BuiltinStatusLine.tsx` helpers
 * (`ui/examples/upstream-patterns/src/components/StatusLine.tsx`,
 * `BuiltinStatusLine.tsx`): we only derive from protocol fields
 * already forwarded to the Lite store — no cost-tracker global, no
 * rate-limit API poll.
 */

/** Count agents currently in the `running` state across the nested
 *  `agentTree`. Shown inline in the header as "N agents". */
export function countRunningAgents(nodes: AgentNode[]): number {
  let total = 0
  for (const node of nodes) {
    if (node.state === 'running') total += 1
    total += countRunningAgents(node.children)
  }
  return total
}

/** Count teams that currently have at least one active member. Shown
 *  inline as "N teams". */
export function countActiveTeams(teams: Record<string, TeamState>): number {
  let total = 0
  for (const team of Object.values(teams)) {
    if (team.members.some(m => m.is_active)) total += 1
  }
  return total
}

/** Count connected MCP servers. */
export function countConnectedMcp(subsystems: SubsystemState): number {
  return subsystems.mcp.filter(s => s.state === 'connected').length
}

/** Count LSP servers currently in the `running` state. */
export function countRunningLsp(subsystems: SubsystemState): number {
  return subsystems.lsp.filter(s => s.state === 'running').length
}

/**
 * Decide whether to render the custom status line row.
 *
 * The backend's `status_line_update` ships a pre-rendered
 * `lines[]`. We only swap in that row when:
 * - a snapshot has been received (`customStatusLine` is non-null)
 * - the last run reported no error, and
 * - at least one non-empty line was emitted.
 *
 * Otherwise we keep the built-in statusline and surface the error
 * separately via `statusLineError`.
 */
export function shouldRenderCustomStatusLine(
  customStatusLine: CustomStatusLineState | null,
): boolean {
  if (!customStatusLine) return false
  if (customStatusLine.error) return false
  if (customStatusLine.lines.length === 0) return false
  return customStatusLine.lines.some(line => line.trim().length > 0)
}

/** The most recent non-empty error reported by the custom statusline
 *  runner, or `null` if the current snapshot is clean. Callers can
 *  surface this as an inline hint on the built-in statusline. */
export function statusLineError(
  customStatusLine: CustomStatusLineState | null,
): string | null {
  if (!customStatusLine) return null
  const trimmed = customStatusLine.error?.trim()
  return trimmed && trimmed.length > 0 ? trimmed : null
}

/** Extract the directory segment shown on the left of the status line
 *  — the last path component of `cwd`, after normalizing backslashes
 *  so the same logic works under PowerShell. */
export function cwdShortName(cwd: string): string {
  if (!cwd) return ''
  const normalized = cwd.replace(/\\/g, '/')
  const parts = normalized.split('/').filter(Boolean)
  return parts[parts.length - 1] ?? cwd
}
