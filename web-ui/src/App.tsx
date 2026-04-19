import { useEffect, useState, useCallback } from 'react'
import { ChatPanel } from '@/components/chat/ChatPanel'
import { Sidebar } from '@/components/sidebar/Sidebar'
import { SessionSidebar } from '@/components/sessions/SessionSidebar'
import { DebugPanel } from '@/components/debug/DebugPanel'
import { useChatStore } from '@/lib/store'
import { fetchAppState, checkConnection, fetchSessions } from '@/lib/api'
import {
  Bug,
  Cpu,
  MessageSquare,
  PanelRightOpen,
  Settings,
  Shield,
  WifiOff,
} from 'lucide-react'

export default function App() {
  const appState = useChatStore((s) => s.appState)
  const debugPanelOpen = useChatStore((s) => s.debugPanelOpen)
  const toggleDebug = useChatStore((s) => s.toggleDebugPanel)
  const setSessions = useChatStore((s) => s.setSessions)
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const [sessionsOpen, setSessionsOpen] = useState(true)
  const [connected, setConnected] = useState(true)

  // Fetch app state on mount + periodic health check
  useEffect(() => {
    fetchAppState()
      .then((state) => {
        useChatStore.getState().setAppState(state)
        setConnected(true)
      })
      .catch(() => setConnected(false))

    // Prime the session sidebar with an initial list so the user can see
    // their history immediately without clicking refresh.
    fetchSessions()
      .then((res) =>
        setSessions({
          sessions: res.sessions,
          currentWorkspace: res.current_workspace,
          activeSessionId: res.active_session_id,
        }),
      )
      .catch(() => { /* sidebar shows its own empty state on failure */ })

    // Health check every 30s
    const interval = setInterval(async () => {
      const ok = await checkConnection()
      setConnected(ok)
      if (ok) {
        // Refresh state on reconnect
        try {
          const state = await fetchAppState()
          useChatStore.getState().setAppState(state)
        } catch { /* ignore */ }
      }
    }, 30000)
    return () => clearInterval(interval)
  }, [setSessions])

  // Ctrl+Shift+D keyboard shortcut for debug panel
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.ctrlKey && e.shiftKey && e.key === 'D') {
      e.preventDefault()
      toggleDebug()
    }
  }, [toggleDebug])

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      {/* Left: session sidebar */}
      <SessionSidebar
        open={sessionsOpen}
        onClose={() => setSessionsOpen(false)}
      />

      {/* Main area */}
      <main className="flex flex-1 flex-col overflow-hidden min-w-0">
        {/* Header bar */}
        <header className="flex h-12 items-center border-b border-border px-3 sm:px-4 gap-2 sm:gap-3 shrink-0">
          <button
            onClick={() => setSessionsOpen((v) => !v)}
            className={`rounded p-1 transition-colors ${
              sessionsOpen ? 'bg-primary/20 text-primary' : 'text-muted-foreground hover:text-foreground'
            }`}
            title={sessionsOpen ? 'Hide sessions' : 'Show sessions'}
          >
            <MessageSquare className="h-4 w-4" />
          </button>
          <h1 className="text-sm font-semibold text-foreground shrink-0">cc-rust</h1>
          <span className="text-xs text-muted-foreground hidden sm:inline">Web UI</span>

          <div className="ml-auto flex items-center gap-1.5 sm:gap-2 text-[11px] text-muted-foreground">
            {/* Connection indicator */}
            {!connected && (
              <span className="flex items-center gap-1 text-red-400" title="Server disconnected">
                <WifiOff className="h-3 w-3" />
                <span className="hidden sm:inline">Offline</span>
              </span>
            )}

            {appState && (
              <>
                <span className="flex items-center gap-1 truncate max-w-32 sm:max-w-none" title={appState.model}>
                  <Cpu className="h-3 w-3 shrink-0" />
                  <span className="truncate">{appState.model}</span>
                </span>
                <span className="flex items-center gap-1 hidden sm:flex">
                  <Shield className="h-3 w-3" />
                  {appState.permission_mode}
                </span>
                {appState.usage && appState.usage.total_cost_usd > 0 && (
                  <span className="font-mono hidden md:inline">${appState.usage.total_cost_usd.toFixed(4)}</span>
                )}
              </>
            )}

            {/* Debug toggle */}
            <button
              onClick={toggleDebug}
              className={`flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors ${
                debugPanelOpen ? 'bg-primary/20 text-primary' : 'hover:text-foreground'
              }`}
              title="Toggle debug panel (Ctrl+Shift+D)"
            >
              <Bug className="h-3 w-3" />
            </button>

            {/* Settings toggle */}
            <button
              onClick={() => setSidebarOpen(!sidebarOpen)}
              className={`flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors ${
                sidebarOpen ? 'bg-primary/20 text-primary' : 'hover:text-foreground'
              }`}
              title="Toggle settings"
            >
              {sidebarOpen ? (
                <PanelRightOpen className="h-3 w-3" />
              ) : (
                <Settings className="h-3 w-3" />
              )}
            </button>
          </div>
        </header>

        <div className="flex flex-1 overflow-hidden">
          {/* Chat panel */}
          <div className="flex flex-1 flex-col overflow-hidden min-w-0">
            <ChatPanel />
          </div>

          {/* Debug panel (tabbed) - hidden on small screens */}
          {debugPanelOpen && (
            <div className="hidden md:flex">
              <DebugPanel />
            </div>
          )}
        </div>
      </main>

      {/* Settings sidebar - hidden on small screens when debug is open */}
      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />
    </div>
  )
}
