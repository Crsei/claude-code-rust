import React, { useEffect, useMemo, useState } from 'react'
import { useAppState } from '../../store/app-store.js'
import type { BackgroundAgent } from '../../store/app-state.js'
import { BackgroundTask } from './BackgroundTask.js'
import { statusOf } from './taskStatusUtils.js'

/**
 * Compact footer-style panel listing current and recently-finished background
 * agents. Adapted from `ui/examples/upstream-patterns/src/components/tasks/
 * BackgroundTaskStatus.tsx`, collapsed to match our single data source
 * (`BackgroundAgent[]`). Running agents are listed first, followed by up to
 * `MAX_RECENT_FINISHED` finished-but-still-interesting rows.
 *
 * Mirrors the visual density of `SubsystemStatus.tsx` (bordered box with a
 * title, small rows beneath). Hides itself entirely when there are no
 * agents to show.
 */

const MAX_RECENT_FINISHED = 3
const RUNNING_TICK_MS = 1000

export function BackgroundTaskStatus() {
  const { backgroundAgents } = useAppState()
  const [now, setNow] = useState(() => Date.now())

  // Sort: running first (oldest first so long-running tasks don't jump to
  // the end), then recently finished (latest first).
  const { running, finished } = useMemo(() => {
    const run: BackgroundAgent[] = []
    const done: BackgroundAgent[] = []
    for (const a of backgroundAgents) {
      if (statusOf(a) === 'running') run.push(a)
      else done.push(a)
    }
    run.sort((a, b) => a.startedAt - b.startedAt)
    done.sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0))
    return {
      running: run,
      finished: done.slice(0, MAX_RECENT_FINISHED),
    }
  }, [backgroundAgents])

  // Tick every second while something is running so elapsed time updates.
  useEffect(() => {
    if (running.length === 0) return
    const id = setInterval(() => setNow(Date.now()), RUNNING_TICK_MS)
    return () => clearInterval(id)
  }, [running.length])

  const totalShown = running.length + finished.length
  if (totalShown === 0) return null

  const runningCount = running.length
  const titleParts = [`${runningCount} running`]
  if (finished.length > 0) titleParts.push(`${finished.length} recent`)
  const title = `Background tasks (${titleParts.join(', ')})`

  return (
    <box
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor="#45475A"
      paddingX={1}
      title={title}
      titleAlignment="left"
      gap={0}
    >
      {running.map(agent => (
        <BackgroundTask key={agent.agentId} agent={agent} now={now} />
      ))}
      {finished.map(agent => (
        <BackgroundTask key={agent.agentId} agent={agent} now={now} />
      ))}
    </box>
  )
}
