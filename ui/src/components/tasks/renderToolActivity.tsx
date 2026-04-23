import React from 'react'
import { c } from '../../theme.js'
import type { ToolActivityRenderItem } from '../../store/message-model.js'
import {
  bashCommandFromActivity,
  ShellProgress,
  toolStatusToTaskStatus,
} from './ShellProgress.js'

/**
 * Compact status chip for a `ToolActivityRenderItem`, used by the tasks-
 * related panels (not the main transcript — `ToolActivityMessage` handles
 * that). Adapted from `ui/examples/upstream-patterns/src/components/tasks/
 * renderToolActivity.tsx`.
 *
 * Upstream pulled a `userFacingName` and `renderToolUseMessage` from a
 * registered `Tools` object. We don't have that registry in the frontend,
 * so we fall back to `name` + `inputSummary` for a single-line summary, and
 * route Bash specifically through `ShellProgress` so shell commands get
 * the friendlier "Running: cmd" formatting.
 */
type Props = {
  item: ToolActivityRenderItem
  maxWidth?: number
}

export function renderToolActivityChip({ item, maxWidth }: Props) {
  const bashCmd = bashCommandFromActivity(item)
  if (bashCmd) {
    return (
      <ShellProgress
        command={bashCmd}
        status={toolStatusToTaskStatus(item.status)}
        maxCommandWidth={maxWidth}
      />
    )
  }

  const summary = item.inputSummary || item.inputDetail || ''
  return (
    <text>
      <span fg={c.warning}>{item.name}</span>
      {summary ? <span fg={c.dim}>({summary})</span> : null}
    </text>
  )
}
