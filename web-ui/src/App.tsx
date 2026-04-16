import { useEffect } from 'react'
import { ChatPanel } from '@/components/chat/ChatPanel'
import { useChatStore } from '@/lib/store'
import { fetchAppState } from '@/lib/api'
import { Cpu, Shield, Bug } from 'lucide-react'

export default function App() {
  const appState = useChatStore((s) => s.appState)
  const debugPanelOpen = useChatStore((s) => s.debugPanelOpen)
  const toggleDebug = useChatStore((s) => s.toggleDebugPanel)
  const rawEvents = useChatStore((s) => s.rawEvents)

  // Fetch app state on mount
  useEffect(() => {
    fetchAppState()
      .then((state) => useChatStore.getState().setAppState(state))
      .catch(console.error)
  }, [])

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      {/* Main chat area */}
      <main className="flex flex-1 flex-col">
        {/* Header bar */}
        <header className="flex h-12 items-center border-b border-border px-4 gap-3">
          <h1 className="text-sm font-semibold text-foreground">cc-rust</h1>
          <span className="text-xs text-muted-foreground">Web UI</span>
          <div className="ml-auto flex items-center gap-3 text-[11px] text-muted-foreground">
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
              </>
            )}
            <button
              onClick={toggleDebug}
              className={`flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors ${
                debugPanelOpen ? 'bg-primary/20 text-primary' : 'hover:text-foreground'
              }`}
              title="Toggle debug panel"
            >
              <Bug className="h-3 w-3" />
              Debug
            </button>
          </div>
        </header>

        <div className="flex flex-1 overflow-hidden">
          {/* Chat panel */}
          <div className="flex flex-1 flex-col overflow-hidden">
            <ChatPanel />
          </div>

          {/* Debug panel — raw SSE event log */}
          {debugPanelOpen && (
            <aside className="w-96 border-l border-border flex flex-col overflow-hidden bg-muted/30">
              <div className="flex items-center justify-between border-b border-border px-3 py-2">
                <span className="text-xs font-medium text-foreground">Raw Events</span>
                <span className="text-[10px] text-muted-foreground">{rawEvents.length}</span>
              </div>
              <div className="flex-1 overflow-y-auto p-2 space-y-1">
                {rawEvents.length === 0 && (
                  <div className="text-xs text-muted-foreground italic py-4 text-center">
                    No events yet
                  </div>
                )}
                {rawEvents.map((evt, i) => (
                  <div key={i} className="rounded border border-border/40 bg-card px-2 py-1.5 text-[10px] font-mono">
                    <div className="flex items-center justify-between mb-0.5">
                      <span className="font-medium text-primary">{evt.event}</span>
                      <span className="text-muted-foreground">
                        {new Date(evt.timestamp).toLocaleTimeString()}
                      </span>
                    </div>
                    <pre className="text-foreground/60 max-h-20 overflow-y-auto whitespace-pre-wrap break-all">
                      {tryTruncateJson(evt.data)}
                    </pre>
                  </div>
                ))}
              </div>
            </aside>
          )}
        </div>
      </main>
    </div>
  )
}

/** Truncate JSON for debug display */
function tryTruncateJson(s: string): string {
  if (s.length <= 200) return s
  try {
    const obj = JSON.parse(s)
    return JSON.stringify(obj, null, 1).slice(0, 300) + '\n...'
  } catch {
    return s.slice(0, 200) + '...'
  }
}
