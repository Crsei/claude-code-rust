import React from 'react'
import type { BackgroundAgent } from '../../store/app-state.js'
import { c } from '../../theme.js'
import {
  elapsedMs,
  formatElapsed,
  getTaskStatusColor,
  getTaskStatusIcon,
  statusOf,
} from './taskStatusUtils.js'

/**
 * Single-row renderer for a background agent. Adapted from the upstream
 * `BackgroundTask.tsx` which branched on `task.type` (local_bash, remote_agent,
 * local_agent, in_process_teammate, local_workflow, monitor_mcp, dream). Our
 * Rust port only exposes the `BackgroundAgent` shape, so there is a single
 * rendering path: icon + description + elapsed + terminal-state hint.
 */
type Props = {
  agent: BackgroundAgent
  now: number
  maxDescriptionWidth?: number
}

function truncate(text: string, max: number): string {
  if (text.length <= max) return text
  if (max <= 1) return text.slice(0, max)
  return `${text.slice(0, max - 1)}\u2026`
}

export function BackgroundTask({ agent, now, maxDescriptionWidth }: Props) {
  const status = statusOf(agent)
  const icon = getTaskStatusIcon(status, { hasError: agent.hadError })
  const color = getTaskStatusColor(status, { hasError: agent.hadError })
  const elapsed = formatElapsed(elapsedMs(agent, now))
  const descLimit = maxDescriptionWidth ?? 60
  const description = truncate(agent.description || agent.agentId, descLimit)

  const stateLabel =
    status === 'running'
      ? 'running'
      : status === 'completed'
        ? 'done'
        : status === 'failed'
          ? 'error'
          : 'stopped'

  return (
    <text>
      <span fg={color}>{icon}</span>
      <span fg={c.text}> {description}</span>
      <span fg={c.dim}>
        {' '}
        ({stateLabel} · {elapsed})
      </span>
    </text>
  )
}
