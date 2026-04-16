import { useChatStore } from '@/lib/store'
import { RawEventLog } from './RawEventLog'
import { MessageInspector } from './MessageInspector'
import { ApiCallTimeline } from './ApiCallTimeline'
import { X } from 'lucide-react'

const TABS = [
  { id: 'events' as const, label: 'Events' },
  { id: 'messages' as const, label: 'Messages' },
  { id: 'timeline' as const, label: 'Timeline' },
]

export function DebugPanel() {
  const debugTab = useChatStore((s) => s.debugTab)
  const setDebugTab = useChatStore((s) => s.setDebugTab)
  const toggleDebug = useChatStore((s) => s.toggleDebugPanel)
  const rawEvents = useChatStore((s) => s.rawEvents)
  const messages = useChatStore((s) => s.messages)
  const lastResult = useChatStore((s) => s.lastResult)

  return (
    <aside className="w-96 border-l border-border flex flex-col overflow-hidden bg-muted/30 shrink-0">
      {/* Header with tabs */}
      <div className="border-b border-border shrink-0">
        <div className="flex items-center justify-between px-3 py-1.5">
          <span className="text-xs font-medium text-foreground">Debug</span>
          <button
            onClick={toggleDebug}
            className="rounded p-0.5 hover:bg-muted transition-colors"
          >
            <X className="h-3 w-3 text-muted-foreground" />
          </button>
        </div>
        <div className="flex px-2 gap-0.5">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setDebugTab(tab.id)}
              className={`px-2.5 py-1 text-[11px] rounded-t-md transition-colors ${
                debugTab === tab.id
                  ? 'bg-background text-foreground font-medium border border-b-0 border-border'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {tab.label}
              {tab.id === 'events' && (
                <span className="ml-1 text-[9px] text-muted-foreground">
                  {rawEvents.length}
                </span>
              )}
              {tab.id === 'messages' && (
                <span className="ml-1 text-[9px] text-muted-foreground">
                  {messages.length}
                </span>
              )}
            </button>
          ))}
        </div>
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {debugTab === 'events' && <RawEventLog events={rawEvents} />}
        {debugTab === 'messages' && <MessageInspector messages={messages} />}
        {debugTab === 'timeline' && <ApiCallTimeline events={rawEvents} lastResult={lastResult} />}
      </div>
    </aside>
  )
}
