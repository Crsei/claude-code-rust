import { dirname } from 'path'
import { existsSync, mkdirSync, readFileSync, watch, type FSWatcher } from 'fs'

export type SubagentEventKind =
  | 'spawn'
  | 'complete'
  | 'error'
  | 'background_complete'
  | 'worktree_kept'
  | 'worktree_cleaned'
  | 'warning'

export type SubagentEvent = {
  ts: string
  kind: SubagentEventKind
  agent_id: string
  parent_agent_id?: string
  description?: string
  model?: string
  depth: number
  background: boolean
  payload?: Record<string, unknown>
}

export type AgentState = {
  agent_id: string
  description?: string
  model?: string
  depth: number
  background: boolean
  status: 'running' | 'completed' | 'error'
  started_at?: string
  ended_at?: string
  duration_ms?: number
  warning?: string
  worktree?: 'kept' | 'cleaned'
}

export type DashboardSnapshot = {
  agents: AgentState[]
  recent_events: SubagentEvent[]
  event_log_path: string
}

export class EventWatcher {
  private readonly eventLogPath: string
  private readonly recentEvents: SubagentEvent[] = []
  private readonly agents = new Map<string, AgentState>()
  private readonly listeners = new Set<(event: SubagentEvent) => void>()
  private watcher?: FSWatcher
  private pollTimer?: ReturnType<typeof setInterval>
  private lastLength = 0
  private scanInFlight = false
  private rescanQueued = false

  constructor(eventLogPath: string) {
    this.eventLogPath = eventLogPath
  }

  start() {
    mkdirSync(dirname(this.eventLogPath), { recursive: true })
    void this.scan()

    try {
      this.watcher = watch(dirname(this.eventLogPath), () => {
        void this.scan()
      })
    } catch (error) {
      console.warn('[subagent-dashboard] fs.watch unavailable:', error)
    }

    this.pollTimer = setInterval(() => {
      void this.scan()
    }, 1000)
  }

  stop() {
    this.watcher?.close()
    if (this.pollTimer) {
      clearInterval(this.pollTimer)
    }
  }

  onEvent(listener: (event: SubagentEvent) => void) {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  getSnapshot(): DashboardSnapshot {
    const agents = Array.from(this.agents.values()).sort((lhs, rhs) => {
      const statusRank = (status: AgentState['status']) =>
        status === 'running' ? 0 : status === 'error' ? 1 : 2
      const rankDiff = statusRank(lhs.status) - statusRank(rhs.status)
      if (rankDiff !== 0) return rankDiff
      return (rhs.started_at ?? '').localeCompare(lhs.started_at ?? '')
    })

    return {
      agents,
      recent_events: [...this.recentEvents],
      event_log_path: this.eventLogPath,
    }
  }

  private async scan() {
    if (this.scanInFlight) {
      this.rescanQueued = true
      return
    }

    this.scanInFlight = true
    try {
      if (!existsSync(this.eventLogPath)) {
        return
      }

      const text = readFileSync(this.eventLogPath, 'utf8')
      if (text.length < this.lastLength) {
        this.lastLength = 0
        this.recentEvents.length = 0
        this.agents.clear()
      }

      const chunk = text.slice(this.lastLength)
      this.lastLength = text.length

      for (const line of chunk.split(/\r?\n/)) {
        const trimmed = line.trim()
        if (!trimmed) continue
        try {
          const event = JSON.parse(trimmed) as SubagentEvent
          this.applyEvent(event)
          for (const listener of this.listeners) {
            listener(event)
          }
        } catch (error) {
          console.warn('[subagent-dashboard] bad event line:', error)
        }
      }
    } finally {
      this.scanInFlight = false
      if (this.rescanQueued) {
        this.rescanQueued = false
        void this.scan()
      }
    }
  }

  private applyEvent(event: SubagentEvent) {
    this.recentEvents.unshift(event)
    if (this.recentEvents.length > 200) {
      this.recentEvents.length = 200
    }

    const current = this.agents.get(event.agent_id) ?? {
      agent_id: event.agent_id,
      description: event.description,
      model: event.model,
      depth: event.depth,
      background: event.background,
      status: 'running' as const,
    }

    const next: AgentState = {
      ...current,
      description: event.description ?? current.description,
      model: event.model ?? current.model,
      depth: event.depth ?? current.depth,
      background: event.background ?? current.background,
    }

    switch (event.kind) {
      case 'spawn':
        next.status = 'running'
        next.started_at = event.ts
        next.ended_at = undefined
        next.duration_ms = undefined
        break
      case 'complete':
        next.status = 'completed'
        next.ended_at = event.ts
        next.duration_ms = asNumber(event.payload?.duration_ms)
        break
      case 'error':
        next.status = 'error'
        next.ended_at = event.ts
        next.duration_ms = asNumber(event.payload?.duration_ms)
        break
      case 'background_complete': {
        const hadError = Boolean(event.payload?.had_error)
        next.status = hadError ? 'error' : 'completed'
        next.ended_at = event.ts
        next.duration_ms = asNumber(event.payload?.duration_ms)
        break
      }
      case 'worktree_kept':
        next.worktree = 'kept'
        break
      case 'worktree_cleaned':
        next.worktree = 'cleaned'
        break
      case 'warning':
        next.warning = String(event.payload?.message ?? 'warning')
        break
    }

    this.agents.set(event.agent_id, next)
  }
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' ? value : undefined
}
