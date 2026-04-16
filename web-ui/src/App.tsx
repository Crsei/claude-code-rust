import { useEffect, useState, useCallback } from 'react'
import { ChatPanel } from '@/components/chat/ChatPanel'
import { Sidebar } from '@/components/sidebar/Sidebar'
import { DebugPanel } from '@/components/debug/DebugPanel'
import { useChatStore } from '@/lib/store'
import { fetchAppState } from '@/lib/api'
import { Cpu, Shield, Bug, Settings, PanelRightOpen } from 'lucide-react'

export default function App() {
  const appState = useChatStore((s) => s.appState)
  const debugPanelOpen = useChatStore((s) => s.debugPanelOpen)
  const toggleDebug = useChatStore((s) => s.toggleDebugPanel)
  const [sidebarOpen, setSidebarOpen] = useState(false)

  // Fetch app state on mount
  useEffect(() => {
    fetchAppState()
      .then((state) => useChatStore.getState().setAppState(state))
      .catch(console.error)
  }, [])

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
      {/* Main area */}
      <main className="flex flex-1 flex-col overflow-hidden">
        {/* Header bar */}
        <header className="flex h-12 items-center border-b border-border px-4 gap-3 shrink-0">
          <h1 className="text-sm font-semibold text-foreground">cc-rust</h1>
          <span className="text-xs text-muted-foreground">Web UI</span>

          <div className="ml-auto flex items-center gap-2 text-[11px] text-muted-foreground">
            {appState && (
              <>
                <span className="flex items-center gap-1">
                  <Cpu className="h-3 w-3" />
                  {appState.model}
                </span>
                <span className="flex items-center gap-1">
                  <Shield className="h-3 w-3" />
                  {appState.permission_mode}
                </span>
                {appState.usage && appState.usage.total_cost_usd > 0 && (
                  <span className="font-mono">${appState.usage.total_cost_usd.toFixed(4)}</span>
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
          <div className="flex flex-1 flex-col overflow-hidden">
            <ChatPanel />
          </div>

          {/* Debug panel (tabbed) */}
          {debugPanelOpen && <DebugPanel />}
        </div>
      </main>

      {/* Settings sidebar */}
      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />
    </div>
  )
}
