import { useEffect, useMemo, useState } from 'react'
import { useChatStore } from '@/lib/store'
import {
  createNewSession,
  fetchAppState,
  fetchSessions,
  resumeSession,
} from '@/lib/api'
import type { SessionSummary } from '@/lib/types'
import {
  ChevronDown,
  ChevronRight,
  FolderOpen,
  MessageSquare,
  Plus,
  RefreshCcw,
  Sparkles,
  X,
} from 'lucide-react'

interface SessionSidebarProps {
  open: boolean
  onClose: () => void
}

interface WorkspaceGroup {
  key: string
  name: string
  root: string
  sessions: SessionSummary[]
  isCurrent: boolean
}

/**
 * Left-hand session navigator — lists all sessions grouped by workspace,
 * with quick entries for starting a new session and for the Playground
 * (a labeled throwaway session). Current-workspace sessions float to the
 * top so they're reachable without scanning.
 */
export function SessionSidebar({ open, onClose }: SessionSidebarProps) {
  const sessions = useChatStore((s) => s.sessions)
  const currentWorkspace = useChatStore((s) => s.currentWorkspace)
  const activeSessionId = useChatStore((s) => s.activeSessionId)
  const loading = useChatStore((s) => s.sessionsLoading)
  const setSessions = useChatStore((s) => s.setSessions)
  const setLoading = useChatStore((s) => s.setSessionsLoading)
  const loadSessionMessages = useChatStore((s) => s.loadSessionMessages)
  const setActiveSessionId = useChatStore((s) => s.setActiveSessionId)
  const clearMessages = useChatStore((s) => s.clearMessages)

  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({})
  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)

  async function refresh() {
    setLoading(true)
    setError(null)
    try {
      const res = await fetchSessions()
      setSessions({
        sessions: res.sessions,
        currentWorkspace: res.current_workspace,
        activeSessionId: res.active_session_id,
      })
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  // Initial load when the sidebar first opens.
  useEffect(() => {
    if (open && sessions.length === 0 && !loading) {
      void refresh()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  async function handleNewSession(playground: boolean) {
    if (pending) return
    setPending(true)
    setError(null)
    try {
      const { session_id } = await createNewSession()
      clearMessages()
      setActiveSessionId(session_id)
      // Refresh app state (session_id on the backend changed).
      try {
        const state = await fetchAppState()
        useChatStore.getState().setAppState(state)
      } catch { /* ignore — state will re-fetch on next heartbeat */ }
      // Refresh session list in the background.
      void refresh()
      if (playground) {
        // Playground starts with a hint so the user can see what the
        // session is for. Kept as a system message so it doesn't get
        // sent to the model until the user types their first real prompt.
        useChatStore.getState().addAssistantMessage({
          id: crypto.randomUUID(),
          role: 'system',
          content:
            'Playground session — experiment freely. Everything here is saved like any other session.',
          timestamp: Date.now(),
        })
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setPending(false)
    }
  }

  async function handleSwitchSession(id: string) {
    if (pending || id === activeSessionId) return
    setPending(true)
    setError(null)
    try {
      const detail = await resumeSession(id)
      loadSessionMessages(detail.messages)
      setActiveSessionId(detail.session_id)
      try {
        const state = await fetchAppState()
        useChatStore.getState().setAppState(state)
      } catch { /* ignore */ }
      void refresh()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setPending(false)
    }
  }

  const groups = useMemo<WorkspaceGroup[]>(() => {
    const byKey = new Map<string, WorkspaceGroup>()
    for (const s of sessions) {
      const key = s.workspace_key || s.cwd
      let g = byKey.get(key)
      if (!g) {
        g = {
          key,
          name: s.workspace_name || s.cwd || 'unknown',
          root: s.workspace_root || s.cwd || '',
          sessions: [],
          isCurrent: currentWorkspace ? currentWorkspace.key === key : false,
        }
        byKey.set(key, g)
      }
      g.sessions.push(s)
    }

    // If we have a current_workspace but no sessions in it yet, still show it.
    if (currentWorkspace && !byKey.has(currentWorkspace.key)) {
      byKey.set(currentWorkspace.key, {
        key: currentWorkspace.key,
        name: currentWorkspace.name,
        root: currentWorkspace.root,
        sessions: [],
        isCurrent: true,
      })
    }

    // Current workspace first, then alphabetical.
    return Array.from(byKey.values()).sort((a, b) => {
      if (a.isCurrent && !b.isCurrent) return -1
      if (b.isCurrent && !a.isCurrent) return 1
      return a.name.localeCompare(b.name)
    })
  }, [sessions, currentWorkspace])

  if (!open) return null

  return (
    <aside className="w-72 border-r border-border flex flex-col overflow-hidden bg-background shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-3 py-3">
        <div className="flex items-center gap-2">
          <MessageSquare className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-medium">Sessions</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => void refresh()}
            disabled={loading || pending}
            className="rounded p-1 hover:bg-muted transition-colors disabled:opacity-40"
            title="Refresh sessions"
          >
            <RefreshCcw className={`h-3.5 w-3.5 text-muted-foreground ${loading ? 'animate-spin' : ''}`} />
          </button>
          <button
            onClick={onClose}
            className="rounded p-1 hover:bg-muted transition-colors"
            title="Close sessions"
          >
            <X className="h-4 w-4 text-muted-foreground" />
          </button>
        </div>
      </div>

      {/* Quick actions */}
      <div className="border-b border-border p-2 space-y-1">
        <button
          onClick={() => void handleNewSession(false)}
          disabled={pending}
          className="w-full flex items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-muted disabled:opacity-50 transition-colors"
        >
          <Plus className="h-3.5 w-3.5 text-primary" />
          <span>New Session</span>
        </button>
        <button
          onClick={() => void handleNewSession(true)}
          disabled={pending}
          className="w-full flex items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-muted disabled:opacity-50 transition-colors"
          title="Start a labeled playground session for quick experiments"
        >
          <Sparkles className="h-3.5 w-3.5 text-amber-400" />
          <span>Playground</span>
        </button>
      </div>

      {/* Error banner */}
      {error && (
        <div className="border-b border-border px-3 py-2 text-[11px] text-red-400 bg-red-500/5">
          {error}
        </div>
      )}

      {/* Session groups */}
      <div className="flex-1 overflow-y-auto">
        {loading && sessions.length === 0 ? (
          <div className="text-xs text-muted-foreground italic text-center py-8 px-3">
            Loading sessions…
          </div>
        ) : groups.length === 0 ? (
          <div className="text-xs text-muted-foreground italic text-center py-8 px-3">
            No sessions yet. Send a message to start one.
          </div>
        ) : (
          groups.map((g) => {
            const isCollapsed = collapsed[g.key] ?? !g.isCurrent
            return (
              <div key={g.key} className="border-b border-border/60 last:border-b-0">
                <button
                  onClick={() =>
                    setCollapsed((prev) => ({ ...prev, [g.key]: !isCollapsed }))
                  }
                  className="w-full flex items-center gap-1.5 px-3 py-2 text-left hover:bg-muted transition-colors"
                  title={g.root}
                >
                  {isCollapsed ? (
                    <ChevronRight className="h-3 w-3 text-muted-foreground shrink-0" />
                  ) : (
                    <ChevronDown className="h-3 w-3 text-muted-foreground shrink-0" />
                  )}
                  <FolderOpen className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                  <span className="text-xs font-medium truncate flex-1">
                    {g.name}
                  </span>
                  {g.isCurrent && (
                    <span className="text-[10px] px-1 py-0.5 rounded bg-primary/20 text-primary shrink-0">
                      current
                    </span>
                  )}
                  <span className="text-[10px] text-muted-foreground shrink-0">
                    {g.sessions.length}
                  </span>
                </button>

                {!isCollapsed && (
                  <ul className="pb-1">
                    {g.sessions.map((s) => (
                      <SessionItem
                        key={s.session_id}
                        session={s}
                        active={s.session_id === activeSessionId}
                        disabled={pending}
                        onClick={() => void handleSwitchSession(s.session_id)}
                      />
                    ))}
                    {g.sessions.length === 0 && (
                      <li className="text-[11px] text-muted-foreground italic px-9 py-1">
                        no saved sessions
                      </li>
                    )}
                  </ul>
                )}
              </div>
            )
          })
        )}
      </div>
    </aside>
  )
}

// ---------------------------------------------------------------------------

interface SessionItemProps {
  session: SessionSummary
  active: boolean
  disabled: boolean
  onClick: () => void
}

function SessionItem({ session, active, disabled, onClick }: SessionItemProps) {
  const label = session.title?.trim() || '(empty session)'
  const ts = formatTimestamp(session.last_modified)

  return (
    <li>
      <button
        onClick={onClick}
        disabled={disabled}
        className={`w-full text-left px-3 py-1.5 pl-9 pr-3 hover:bg-muted transition-colors disabled:opacity-60 ${
          active ? 'bg-primary/10 border-l-2 border-primary' : ''
        }`}
        title={`${session.session_id}\n${session.cwd}`}
      >
        <div className="text-xs text-foreground truncate">{label}</div>
        <div className="text-[10px] text-muted-foreground flex items-center gap-2">
          <span>{ts}</span>
          <span>·</span>
          <span>{session.message_count} msgs</span>
        </div>
      </button>
    </li>
  )
}

function formatTimestamp(unixSeconds: number): string {
  if (!unixSeconds) return 'unknown'
  const d = new Date(unixSeconds * 1000)
  const now = new Date()
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()

  if (sameDay) {
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  }
  return d.toLocaleDateString([], { month: 'short', day: 'numeric' })
}
