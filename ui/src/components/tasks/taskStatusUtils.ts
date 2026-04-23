/**
 * Shared utilities for displaying task status across different task types.
 *
 * Adapted from `ui/examples/upstream-patterns/src/components/tasks/taskStatusUtils.tsx`.
 * The upstream references a rich `TaskState` union from `src/tasks/types.ts`
 * that we don't have in the Rust port — here we derive the same status set
 * from `BackgroundAgent` fields in our store (`app-state.ts`):
 *
 *   - completedAt unset                       → 'running'
 *   - completedAt set && hadError             → 'failed'
 *   - completedAt set && !hadError            → 'completed'
 *   - (reserved) explicitly aborted           → 'killed'
 *
 * Glyphs are unicode characters rather than the upstream `figures` package,
 * and colors are pulled from `theme.ts` as hex strings so they can be
 * consumed directly by OpenTUI `<text fg=...>` attributes.
 */
import type { BackgroundAgent } from '../../store/app-state.js'
import { c } from '../../theme.js'

export type TaskStatus = 'running' | 'completed' | 'failed' | 'killed'

/**
 * Returns true if the given task status represents a terminal (finished) state.
 */
export function isTerminalStatus(status: TaskStatus): boolean {
  return status === 'completed' || status === 'failed' || status === 'killed'
}

export interface TaskStatusOptions {
  isIdle?: boolean
  awaitingApproval?: boolean
  hasError?: boolean
  shutdownRequested?: boolean
}

/**
 * Derive a status from a `BackgroundAgent` record.
 */
export function statusOf(agent: BackgroundAgent): TaskStatus {
  if (agent.completedAt == null) return 'running'
  if (agent.hadError) return 'failed'
  return 'completed'
}

/**
 * Returns the appropriate unicode icon for a task based on status and state flags.
 */
export function getTaskStatusIcon(
  status: TaskStatus,
  options?: TaskStatusOptions,
): string {
  const { isIdle, awaitingApproval, hasError, shutdownRequested } =
    options ?? {}

  if (hasError) return '\u2717' // ✗
  if (awaitingApproval) return '?'
  if (shutdownRequested) return '\u26A0' // ⚠

  if (status === 'running') {
    if (isIdle) return '\u2026' // …
    return '\u25B6' // ▶
  }
  if (status === 'completed') return '\u2713' // ✓
  if (status === 'failed' || status === 'killed') return '\u2717' // ✗
  return '\u25CF' // ●
}

/**
 * Returns a hex color for a task based on status and state flags.
 * Uses the palette from `theme.ts` so colors stay consistent with other panels.
 */
export function getTaskStatusColor(
  status: TaskStatus,
  options?: TaskStatusOptions,
): string {
  const { isIdle, awaitingApproval, hasError, shutdownRequested } =
    options ?? {}

  if (hasError) return c.error
  if (awaitingApproval) return c.warning
  if (shutdownRequested) return c.warning
  if (isIdle) return c.dim

  if (status === 'completed') return c.success
  if (status === 'failed') return c.error
  if (status === 'killed') return c.warning
  return c.info
}

/**
 * Format an elapsed duration in milliseconds as a compact human-readable
 * string. Examples: `0.4s`, `12s`, `3m 05s`, `1h 02m`.
 */
export function formatElapsed(ms: number): string {
  if (ms < 0) ms = 0
  if (ms < 1000) {
    const s = (ms / 1000).toFixed(1)
    return `${s}s`
  }
  const totalSec = Math.floor(ms / 1000)
  if (totalSec < 60) return `${totalSec}s`
  const minutes = Math.floor(totalSec / 60)
  const seconds = totalSec % 60
  if (minutes < 60) {
    return `${minutes}m ${seconds.toString().padStart(2, '0')}s`
  }
  const hours = Math.floor(minutes / 60)
  const mins = minutes % 60
  return `${hours}h ${mins.toString().padStart(2, '0')}m`
}

/**
 * Compute the elapsed runtime for an agent, in milliseconds.
 * Running tasks use `now`; finished tasks use `completedAt` or `durationMs`.
 */
export function elapsedMs(agent: BackgroundAgent, now: number): number {
  if (agent.completedAt != null) {
    return Math.max(0, agent.completedAt - agent.startedAt)
  }
  if (agent.durationMs != null) {
    return agent.durationMs
  }
  return Math.max(0, now - agent.startedAt)
}
