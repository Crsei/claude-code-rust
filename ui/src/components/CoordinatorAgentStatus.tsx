import React, { useEffect, useState } from 'react'
import { c } from '../theme.js'
import { formatNumber } from '../utils.js'
import { formatDuration } from './shell/format.js'

/**
 * Coordinator panel \u2014 steerable list of local agent tasks shown below
 * the prompt input footer when any `local_agent` tasks exist.
 *
 * OpenTUI-native port of the upstream `CoordinatorAgentStatus`
 * (`ui/examples/upstream-patterns/src/components/CoordinatorAgentStatus.tsx`).
 * Upstream pulled state from a task framework this repo doesn't ship
 * (`tasks/LocalAgentTask`), so this port accepts `CoordinatorTask`s
 * and a selected-index directly \u2014 parity with the upstream visual
 * contract, no hidden dependency on the teammate-view helpers.
 */

const BLACK_CIRCLE = '\u25CF'
const CIRCLE = '\u25CB'
const POINTER = '\u276F'
const PLAY_ICON = '\u25B6'
const PAUSE_ICON = '\u23F8'
const ARROW_UP = '\u2191'
const ARROW_DOWN = '\u2193'

export type CoordinatorTaskStatus = 'running' | 'paused' | 'completed' | 'failed'

export type CoordinatorTask = {
  id: string
  name?: string
  description: string
  status: CoordinatorTaskStatus
  startTime: number
  endTime?: number
  totalPausedMs?: number
  progress?: {
    summary?: string
    tokenCount?: number
    lastActivity?: 'input' | 'output'
  }
  pendingMessages?: Array<unknown>
  evictAfter?: number
}

type Props = {
  tasks: CoordinatorTask[]
  /**
   * Index into the logical list starting with the synthetic "main" row
   * at `0`. Pass `undefined` to render without any highlight.
   */
  selectedIndex?: number
  /** The task ID currently being viewed (drives the `BLACK_CIRCLE` bullet). */
  viewingTaskId?: string
  onSelectMain?: () => void
  onSelectTask?: (task: CoordinatorTask) => void
}

function isRunning(status: CoordinatorTaskStatus): boolean {
  return status === 'running' || status === 'paused'
}

function useNowTick(shouldTick: boolean): void {
  const [, setTick] = useState(0)
  useEffect(() => {
    if (!shouldTick) return
    const id = setInterval(() => setTick(t => t + 1), 1000)
    return () => clearInterval(id)
  }, [shouldTick])
}

export function CoordinatorAgentStatus({
  tasks,
  selectedIndex,
  viewingTaskId,
  onSelectMain,
  onSelectTask,
}: Props) {
  const hasRunning = tasks.some(t => isRunning(t.status))
  useNowTick(hasRunning)

  const visibleTasks = tasks.filter(t => t.evictAfter !== 0)
  if (visibleTasks.length === 0) return null

  return (
    <box flexDirection="column" marginTop={1}>
      <MainLine
        isSelected={selectedIndex === 0}
        isViewed={viewingTaskId === undefined}
        onClick={onSelectMain}
      />
      {visibleTasks.map((task, i) => (
        <AgentLine
          key={task.id}
          task={task}
          isSelected={selectedIndex === i + 1}
          isViewed={viewingTaskId === task.id}
          onClick={onSelectTask ? () => onSelectTask(task) : undefined}
        />
      ))}
    </box>
  )
}

function MainLine({
  isSelected,
  isViewed,
  onClick,
}: {
  isSelected?: boolean
  isViewed?: boolean
  onClick?: () => void
}) {
  const bullet = isViewed ? BLACK_CIRCLE : CIRCLE
  const prefix = isSelected ? `${POINTER} ` : '  '
  const dim = !isSelected && !isViewed
  const content = (
    <text fg={dim ? c.dim : undefined}>
      {isViewed ? (
        <strong>
          {prefix}
          {bullet} main
        </strong>
      ) : (
        <>
          {prefix}
          {bullet} main
        </>
      )}
    </text>
  )
  if (!onClick) return content
  return <box>{content}</box>
}

function AgentLine({
  task,
  isSelected,
  isViewed,
  onClick,
}: {
  task: CoordinatorTask
  isSelected?: boolean
  isViewed?: boolean
  onClick?: () => void
}) {
  const running = isRunning(task.status)
  const pausedMs = task.totalPausedMs ?? 0
  const elapsedMs = Math.max(
    0,
    running
      ? Date.now() - task.startTime - pausedMs
      : (task.endTime ?? task.startTime) - task.startTime - pausedMs,
  )
  const elapsed = formatDuration(elapsedMs)
  const tokenCount = task.progress?.tokenCount
  const lastActivity = task.progress?.lastActivity
  const arrow = lastActivity === 'output' ? ARROW_DOWN : ARROW_UP
  const tokenText =
    tokenCount !== undefined && tokenCount > 0
      ? ` \u00B7 ${arrow} ${formatNumber(tokenCount)} tokens`
      : ''
  const queuedCount = task.pendingMessages?.length ?? 0
  const queuedText = queuedCount > 0 ? ` \u00B7 ${queuedCount} queued` : ''
  const displayDescription = task.progress?.summary || task.description
  const bullet = isViewed ? BLACK_CIRCLE : CIRCLE
  const prefix = isSelected ? `${POINTER} ` : '  '
  const sep = task.status === 'paused' ? PAUSE_ICON : running ? PLAY_ICON : BLACK_CIRCLE
  const dim = !isSelected && !isViewed
  const hint =
    isSelected && !isViewed
      ? ` \u00B7 x to ${running ? 'stop' : 'clear'}`
      : ''

  const content = (
    <text fg={dim ? c.dim : undefined}>
      {isViewed ? (
        <strong>
          {prefix}
          {bullet}{' '}
          {task.name && (
            <>
              <span>{task.name}</span>
              {': '}
            </>
          )}
          {displayDescription} {sep} {elapsed}
        </strong>
      ) : (
        <>
          {prefix}
          {bullet}{' '}
          {task.name && (
            <strong>
              <span>{task.name}: </span>
            </strong>
          )}
          {displayDescription} {sep} {elapsed}
        </>
      )}
      {tokenText}
      {queuedCount > 0 && <span fg={c.warning}>{queuedText}</span>}
      {hint && <span fg={c.dim}>{hint}</span>}
    </text>
  )

  if (!onClick) return content
  return <box>{content}</box>
}
