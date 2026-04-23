import React, { useEffect, useRef, useState } from 'react'
import { c } from '../theme.js'

/**
 * Ported from `ui/examples/upstream-patterns/src/components/TaskListV2.tsx`.
 *
 * Upstream reads tasks + teammate metadata from `useAppState`, accesses
 * the `isTodoV2Enabled` feature gate, and uses Ink's `useTerminalSize`
 * to decide how many tasks to show. The Rust port accepts tasks via
 * props so this file doesn't need to know about the background-agent
 * reducers; `isTodoV2Enabled` and the teammate swarm integration are
 * either gated at the caller or represented with optional fields.
 *
 * Layout decisions preserved:
 *  - Truncation cap: min 3, max 10, 14 rows of headroom on small windows.
 *  - Completion TTL: 30 seconds — recently completed tasks stay visible
 *    before older completions collapse into the hidden summary.
 *  - Prioritised visible order: recent completed \u2192 in_progress \u2192
 *    pending (blockers last) \u2192 older completed.
 *  - Blocked indicators show `blocked by #id` in dim text.
 *  - Owner "(@name)" suffix hides below 60 columns, matching upstream.
 */

export type TaskV2Status = 'pending' | 'in_progress' | 'completed'

export type TaskV2 = {
  id: string
  subject: string
  status: TaskV2Status
  /** Other task IDs that must be complete before this one can start. */
  blockedBy: string[]
  /** Optional teammate handle (e.g. "researcher" or "researcher@team"). */
  owner?: string
}

export type TeammateInfo = {
  name: string
  color?: string
  /** Is this teammate still running? */
  active?: boolean
  /** Latest activity line to surface under running tasks owned by them. */
  activity?: string
}

type Props = {
  tasks: TaskV2[]
  /** When true, renders the summary header and outer margins. */
  isStandalone?: boolean
  /** Optional teammate lookup keyed by `agentName` OR `agentId`. */
  teammates?: Record<string, TeammateInfo>
  /** Current terminal width (columns). Defaults to 80. */
  columns?: number
  /** Current terminal height (rows). Defaults to 24. */
  rows?: number
}

const RECENT_COMPLETED_TTL_MS = 30_000
const TICK = '\u2713'
const FILLED_SQUARE = '\u25AA'
const EMPTY_SQUARE = '\u25AB'
const POINTER_SMALL = '\u203A'
const ELLIPSIS = '\u2026'

function count<T>(values: T[], predicate: (v: T) => boolean): number {
  let n = 0
  for (const value of values) if (predicate(value)) n += 1
  return n
}

function byIdAsc(a: TaskV2, b: TaskV2): number {
  const ai = parseInt(a.id, 10)
  const bi = parseInt(b.id, 10)
  if (!Number.isNaN(ai) && !Number.isNaN(bi)) return ai - bi
  return a.id.localeCompare(b.id)
}

function truncateToWidth(input: string, width: number): string {
  if (input.length <= width) return input
  if (width <= 1) return input.slice(0, width)
  return input.slice(0, width - 1) + ELLIPSIS
}

function getTaskIcon(status: TaskV2Status): {
  icon: string
  color: string | undefined
} {
  switch (status) {
    case 'completed':
      return { icon: TICK, color: c.success }
    case 'in_progress':
      return { icon: FILLED_SQUARE, color: c.accent }
    case 'pending':
      return { icon: EMPTY_SQUARE, color: undefined }
  }
}

export function TaskListV2({
  tasks,
  isStandalone = false,
  teammates,
  columns = 80,
  rows = 24,
}: Props): React.ReactElement | null {
  const [, forceUpdate] = useState(0)
  const completionTimestampsRef = useRef(new Map<string, number>())
  const previousCompletedIdsRef = useRef<Set<string> | null>(null)

  if (previousCompletedIdsRef.current === null) {
    previousCompletedIdsRef.current = new Set(
      tasks.filter(t => t.status === 'completed').map(t => t.id),
    )
  }

  const maxDisplay = rows <= 10 ? 0 : Math.min(10, Math.max(3, rows - 14))

  const currentCompletedIds = new Set(
    tasks.filter(t => t.status === 'completed').map(t => t.id),
  )
  const now = Date.now()
  for (const id of currentCompletedIds) {
    if (!previousCompletedIdsRef.current.has(id)) {
      completionTimestampsRef.current.set(id, now)
    }
  }
  for (const id of Array.from(completionTimestampsRef.current.keys())) {
    if (!currentCompletedIds.has(id)) {
      completionTimestampsRef.current.delete(id)
    }
  }
  previousCompletedIdsRef.current = currentCompletedIds

  useEffect(() => {
    if (completionTimestampsRef.current.size === 0) return
    const currentNow = Date.now()
    let earliestExpiry = Infinity
    for (const ts of completionTimestampsRef.current.values()) {
      const expiry = ts + RECENT_COMPLETED_TTL_MS
      if (expiry > currentNow && expiry < earliestExpiry) {
        earliestExpiry = expiry
      }
    }
    if (!Number.isFinite(earliestExpiry)) return
    const timer = setTimeout(
      () => forceUpdate(n => n + 1),
      earliestExpiry - currentNow,
    )
    return () => clearTimeout(timer)
  }, [tasks])

  if (tasks.length === 0) return null

  const completedCount = count(tasks, t => t.status === 'completed')
  const pendingCount = count(tasks, t => t.status === 'pending')
  const inProgressCount = tasks.length - completedCount - pendingCount
  const unresolvedTaskIds = new Set(
    tasks.filter(t => t.status !== 'completed').map(t => t.id),
  )

  const needsTruncation = tasks.length > maxDisplay

  let visibleTasks: TaskV2[]
  let hiddenTasks: TaskV2[]

  if (needsTruncation) {
    const recentCompleted: TaskV2[] = []
    const olderCompleted: TaskV2[] = []
    for (const task of tasks.filter(t => t.status === 'completed')) {
      const ts = completionTimestampsRef.current.get(task.id)
      if (ts && now - ts < RECENT_COMPLETED_TTL_MS) {
        recentCompleted.push(task)
      } else {
        olderCompleted.push(task)
      }
    }
    recentCompleted.sort(byIdAsc)
    olderCompleted.sort(byIdAsc)
    const inProgress = tasks
      .filter(t => t.status === 'in_progress')
      .sort(byIdAsc)
    const pending = tasks
      .filter(t => t.status === 'pending')
      .sort((a, b) => {
        const aBlocked = a.blockedBy.some(id => unresolvedTaskIds.has(id))
        const bBlocked = b.blockedBy.some(id => unresolvedTaskIds.has(id))
        if (aBlocked !== bBlocked) return aBlocked ? 1 : -1
        return byIdAsc(a, b)
      })
    const prioritized = [
      ...recentCompleted,
      ...inProgress,
      ...pending,
      ...olderCompleted,
    ]
    visibleTasks = prioritized.slice(0, maxDisplay)
    hiddenTasks = prioritized.slice(maxDisplay)
  } else {
    visibleTasks = [...tasks].sort(byIdAsc)
    hiddenTasks = []
  }

  let hiddenSummary = ''
  if (hiddenTasks.length > 0) {
    const parts: string[] = []
    const hiddenPending = count(hiddenTasks, t => t.status === 'pending')
    const hiddenInProgress = count(hiddenTasks, t => t.status === 'in_progress')
    const hiddenCompleted = count(hiddenTasks, t => t.status === 'completed')
    if (hiddenInProgress > 0) parts.push(`${hiddenInProgress} in progress`)
    if (hiddenPending > 0) parts.push(`${hiddenPending} pending`)
    if (hiddenCompleted > 0) parts.push(`${hiddenCompleted} completed`)
    hiddenSummary = ` \u2026 +${parts.join(', ')}`
  }

  const content = (
    <>
      {visibleTasks.map(task => {
        const owner = task.owner ? teammates?.[task.owner] : undefined
        return (
          <TaskItem
            key={task.id}
            task={task}
            ownerInfo={owner}
            openBlockers={task.blockedBy.filter(id => unresolvedTaskIds.has(id))}
            columns={columns}
          />
        )
      })}
      {maxDisplay > 0 && hiddenSummary && (
        <text fg={c.dim}>{hiddenSummary}</text>
      )}
    </>
  )

  if (isStandalone) {
    return (
      <box flexDirection="column" marginTop={1} marginLeft={2}>
        <box flexDirection="row">
          <text fg={c.dim}>
            <strong>{tasks.length}</strong>
            {' tasks ('}
            <strong>{completedCount}</strong>
            {' done, '}
            {inProgressCount > 0 ? (
              <>
                <strong>{inProgressCount}</strong>
                {' in progress, '}
              </>
            ) : null}
            <strong>{pendingCount}</strong>
            {' open)'}
          </text>
        </box>
        {content}
      </box>
    )
  }

  return <box flexDirection="column">{content}</box>
}

type TaskItemProps = {
  task: TaskV2
  ownerInfo?: TeammateInfo
  openBlockers: string[]
  columns: number
}

function TaskItem({
  task,
  ownerInfo,
  openBlockers,
  columns,
}: TaskItemProps): React.ReactElement {
  const isCompleted = task.status === 'completed'
  const isInProgress = task.status === 'in_progress'
  const isBlocked = openBlockers.length > 0
  const { icon, color } = getTaskIcon(task.status)

  const ownerActive = ownerInfo?.active !== false && !!task.owner
  const showOwner = columns >= 60 && !!task.owner && ownerActive
  const ownerSuffix = showOwner ? ` (@${task.owner})` : ''
  const maxSubjectWidth = Math.max(15, columns - 15 - ownerSuffix.length)
  const displaySubject = truncateToWidth(task.subject, maxSubjectWidth)
  const showActivity = isInProgress && !isBlocked && !!ownerInfo?.activity
  const maxActivityWidth = Math.max(15, columns - 15)
  const displayActivity = ownerInfo?.activity
    ? truncateToWidth(ownerInfo.activity, maxActivityWidth)
    : undefined

  return (
    <box flexDirection="column">
      <box flexDirection="row">
        <text fg={color}>{icon} </text>
        <text
          fg={isCompleted || isBlocked ? c.dim : undefined}
          strikethrough={isCompleted}
        >
          {isInProgress ? <strong>{displaySubject}</strong> : displaySubject}
        </text>
        {showOwner ? (
          <text fg={c.dim}>
            {' ('}
            <span fg={ownerInfo?.color ?? c.dim}>@{task.owner}</span>
            {')'}
          </text>
        ) : null}
        {isBlocked ? (
          <text fg={c.dim}>
            {' '}
            {POINTER_SMALL} blocked by{' '}
            {[...openBlockers]
              .sort((a, b) => parseInt(a, 10) - parseInt(b, 10))
              .map(id => `#${id}`)
              .join(', ')}
          </text>
        ) : null}
      </box>
      {showActivity && displayActivity ? (
        <box flexDirection="row">
          <text fg={c.dim}>
            {'  '}
            {displayActivity}
            {ELLIPSIS}
          </text>
        </box>
      ) : null}
    </box>
  )
}
