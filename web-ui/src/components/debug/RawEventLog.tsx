import { useState, useMemo } from 'react'
import { Search, Filter } from 'lucide-react'

interface RawEvent {
  timestamp: number
  event: string
  data: string
}

interface RawEventLogProps {
  events: RawEvent[]
}

const EVENT_COLORS: Record<string, string> = {
  system_init: 'text-blue-400',
  assistant: 'text-green-400',
  stream_event: 'text-yellow-400',
  result: 'text-purple-400',
  api_retry: 'text-red-400',
  tool_use_summary: 'text-cyan-400',
  user_replay: 'text-orange-400',
  compact_boundary: 'text-pink-400',
}

export function RawEventLog({ events }: RawEventLogProps) {
  const [search, setSearch] = useState('')
  const [filterType, setFilterType] = useState<string | null>(null)
  const [expandedIdx, setExpandedIdx] = useState<number | null>(null)

  // Get unique event types for filter
  const eventTypes = useMemo(() => {
    const types = new Set(events.map(e => e.event))
    return Array.from(types).sort()
  }, [events])

  const filtered = useMemo(() => {
    let result = events
    if (filterType) {
      result = result.filter(e => e.event === filterType)
    }
    if (search) {
      const q = search.toLowerCase()
      result = result.filter(e =>
        e.event.toLowerCase().includes(q) ||
        e.data.toLowerCase().includes(q)
      )
    }
    return result
  }, [events, search, filterType])

  return (
    <div className="flex flex-col h-full">
      {/* Search + filter bar */}
      <div className="px-2 py-1.5 border-b border-border/40 space-y-1 shrink-0">
        <div className="relative">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Filter events..."
            className="w-full rounded bg-muted/50 pl-7 pr-2 py-1 text-[11px] text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary border border-border/40"
          />
        </div>
        <div className="flex flex-wrap gap-0.5">
          <button
            onClick={() => setFilterType(null)}
            className={`px-1.5 py-0.5 rounded text-[9px] transition-colors ${
              !filterType ? 'bg-primary/20 text-primary' : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            All
          </button>
          {eventTypes.map(type => (
            <button
              key={type}
              onClick={() => setFilterType(filterType === type ? null : type)}
              className={`px-1.5 py-0.5 rounded text-[9px] transition-colors ${
                filterType === type ? 'bg-primary/20 text-primary' : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {type}
            </button>
          ))}
        </div>
      </div>

      {/* Event list */}
      <div className="flex-1 overflow-y-auto p-1.5 space-y-0.5">
        {filtered.length === 0 && (
          <div className="text-xs text-muted-foreground italic py-4 text-center">
            {events.length === 0 ? 'No events yet' : 'No matching events'}
          </div>
        )}
        {filtered.map((evt, i) => {
          const isExpanded = expandedIdx === i
          return (
            <button
              key={i}
              onClick={() => setExpandedIdx(isExpanded ? null : i)}
              className="w-full text-left rounded border border-border/30 bg-card px-2 py-1 text-[10px] font-mono hover:border-border/60 transition-colors"
            >
              <div className="flex items-center justify-between">
                <span className={`font-medium ${EVENT_COLORS[evt.event] || 'text-foreground'}`}>
                  {evt.event}
                </span>
                <span className="text-muted-foreground/60">
                  {new Date(evt.timestamp).toLocaleTimeString()}
                </span>
              </div>
              <pre className={`text-foreground/50 whitespace-pre-wrap break-all ${
                isExpanded ? 'max-h-96' : 'max-h-12'
              } overflow-y-auto mt-0.5`}>
                {isExpanded ? tryPrettyJson(evt.data) : truncate(evt.data, 100)}
              </pre>
            </button>
          )
        })}
      </div>
    </div>
  )
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n) + '...'
}

function tryPrettyJson(s: string): string {
  try {
    return JSON.stringify(JSON.parse(s), null, 2)
  } catch {
    return s
  }
}
