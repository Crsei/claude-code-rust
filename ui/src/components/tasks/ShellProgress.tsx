import React from 'react'
import { c } from '../../theme.js'
import type { ToolActivityRenderItem } from '../../store/message-model.js'
import {
  getTaskStatusColor,
  getTaskStatusIcon,
  type TaskStatus,
} from './taskStatusUtils.js'

/**
 * Helpers for rendering a running shell (Bash tool activity) as a compact
 * "Running: <cmd>" line. Adapted from `ui/examples/upstream-patterns/src/
 * components/tasks/ShellProgress.tsx`.
 *
 * Upstream had a `LocalShellTaskState` type from the backend task system;
 * we instead accept either a raw command string or a `ToolActivityRenderItem`
 * produced by our message model (Bash tool_use entries have `input.command`).
 */

type TaskStatusTextProps = {
  status: TaskStatus
  label?: string
  suffix?: string
}

export function TaskStatusText({
  status,
  label,
  suffix,
}: TaskStatusTextProps) {
  const displayLabel = label ?? status
  const color = getTaskStatusColor(status)
  return (
    <text>
      <span fg={color}>(</span>
      <span fg={color}>{displayLabel}</span>
      {suffix ? <span fg={color}>{suffix}</span> : null}
      <span fg={color}>)</span>
    </text>
  )
}

type ShellProgressProps = {
  command: string
  status: TaskStatus
  maxCommandWidth?: number
}

function truncate(text: string, max: number): string {
  if (text.length <= max) return text
  if (max <= 1) return text.slice(0, max)
  return `${text.slice(0, max - 1)}\u2026`
}

/**
 * Compact one-liner like `▶ Running: git status (running)` for a shell
 * command. Colors follow task-status semantics.
 */
export function ShellProgress({
  command,
  status,
  maxCommandWidth,
}: ShellProgressProps) {
  const icon = getTaskStatusIcon(status)
  const color = getTaskStatusColor(status)
  const trimmed = truncate(command, maxCommandWidth ?? 80)
  const label =
    status === 'running'
      ? 'Running'
      : status === 'completed'
        ? 'Done'
        : status === 'failed'
          ? 'Failed'
          : 'Stopped'

  return (
    <text>
      <span fg={color}>{icon}</span>
      <span fg={c.dim}> {label}: </span>
      <span fg={c.text}>{trimmed}</span>
    </text>
  )
}

/**
 * Derive the Bash command (if any) from a `ToolActivityRenderItem`.
 * Returns `undefined` for non-Bash tool activities so callers can fall back
 * to more generic rendering.
 */
export function bashCommandFromActivity(
  item: ToolActivityRenderItem,
): string | undefined {
  if (item.name !== 'Bash') return undefined
  const cmd = item.input?.command
  return typeof cmd === 'string' && cmd.length > 0 ? cmd : undefined
}

/**
 * Map a `ToolActivityRenderItem.status` onto our `TaskStatus` set.
 */
export function toolStatusToTaskStatus(
  s: ToolActivityRenderItem['status'],
): TaskStatus {
  switch (s) {
    case 'success':
      return 'completed'
    case 'error':
      return 'failed'
    case 'cancelled':
      return 'killed'
    case 'pending':
    case 'running':
    default:
      return 'running'
  }
}
