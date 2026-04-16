import { useMemo } from 'react'
import type { ResultEvent } from '@/lib/types'
import { Clock, Zap, AlertTriangle, CheckCircle2, XCircle } from 'lucide-react'

interface RawEvent {
  timestamp: number
  event: string
  data: string
}

interface ApiCallTimelineProps {
  events: RawEvent[]
  lastResult: ResultEvent | null
}

interface TimelineEntry {
  type: 'api_call' | 'tool' | 'retry' | 'result'
  timestamp: number
  label: string
  detail: string
  duration?: number
  status?: 'success' | 'error' | 'pending'
}

export function ApiCallTimeline({ events, lastResult }: ApiCallTimelineProps) {
  const timeline = useMemo(() => buildTimeline(events, lastResult), [events, lastResult])

  const firstTs = timeline.length > 0 ? timeline[0].timestamp : 0

  return (
    <div className="flex-1 overflow-y-auto p-2">
      {timeline.length === 0 && (
        <div className="text-xs text-muted-foreground italic py-4 text-center">
          No API activity yet
        </div>
      )}

      {/* Result summary card */}
      {lastResult && <ResultSummary result={lastResult} />}

      {/* Timeline */}
      <div className="relative ml-3 border-l border-border/40 pl-4 space-y-2 mt-3">
        {timeline.map((entry, i) => (
          <div key={i} className="relative">
            {/* Dot */}
            <div className={`absolute -left-[21px] top-1 h-2.5 w-2.5 rounded-full border-2 border-background ${
              entry.status === 'error' ? 'bg-red-500' :
              entry.status === 'success' ? 'bg-green-500' :
              entry.type === 'retry' ? 'bg-yellow-500' :
              'bg-blue-500'
            }`} />

            {/* Content */}
            <div className="text-[10px]">
              <div className="flex items-center gap-1.5">
                <span className="text-muted-foreground/50 font-mono w-14 shrink-0">
                  +{((entry.timestamp - firstTs) / 1000).toFixed(1)}s
                </span>
                <span className={`font-medium ${
                  entry.type === 'result' ? 'text-purple-400' :
                  entry.type === 'retry' ? 'text-yellow-400' :
                  entry.type === 'tool' ? 'text-cyan-400' :
                  'text-foreground/80'
                }`}>
                  {entry.label}
                </span>
                {entry.duration != null && (
                  <span className="text-muted-foreground/50">
                    {entry.duration}ms
                  </span>
                )}
              </div>
              {entry.detail && (
                <div className="text-muted-foreground/50 ml-14 mt-0.5 font-mono truncate">
                  {entry.detail}
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

function ResultSummary({ result }: { result: ResultEvent }) {
  const isError = result.is_error
  const Icon = isError ? XCircle : CheckCircle2

  return (
    <div className={`rounded-lg border p-3 space-y-2 ${
      isError ? 'border-red-500/30 bg-red-950/20' : 'border-green-500/30 bg-green-950/20'
    }`}>
      <div className="flex items-center gap-2">
        <Icon className={`h-4 w-4 ${isError ? 'text-red-400' : 'text-green-400'}`} />
        <span className={`text-xs font-medium ${isError ? 'text-red-400' : 'text-green-400'}`}>
          {result.subtype.replace(/_/g, ' ')}
        </span>
      </div>

      <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-[10px]">
        <div className="flex items-center gap-1 text-muted-foreground">
          <Clock className="h-3 w-3" />
          <span>Total: {(result.duration_ms / 1000).toFixed(1)}s</span>
        </div>
        <div className="flex items-center gap-1 text-muted-foreground">
          <Zap className="h-3 w-3" />
          <span>API: {(result.duration_api_ms / 1000).toFixed(1)}s</span>
        </div>
        <div className="text-muted-foreground">
          Turns: {result.num_turns}
        </div>
        <div className="text-muted-foreground font-mono">
          ${result.total_cost_usd.toFixed(4)}
        </div>
        <div className="text-blue-400">
          in: {result.usage.total_input_tokens.toLocaleString()}
        </div>
        <div className="text-green-400">
          out: {result.usage.total_output_tokens.toLocaleString()}
        </div>
      </div>

      {result.errors.length > 0 && (
        <div className="space-y-0.5">
          {result.errors.map((err, i) => (
            <div key={i} className="flex items-start gap-1 text-[10px] text-red-300">
              <AlertTriangle className="h-3 w-3 shrink-0 mt-0.5" />
              <span>{err}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

function buildTimeline(events: RawEvent[], lastResult: ResultEvent | null): TimelineEntry[] {
  const entries: TimelineEntry[] = []

  for (const evt of events) {
    try {
      const data = JSON.parse(evt.data)

      switch (evt.event) {
        case 'system_init':
          entries.push({
            type: 'api_call',
            timestamp: evt.timestamp,
            label: 'Session init',
            detail: `model: ${data.model || '?'}, tools: ${data.tools?.length || 0}`,
            status: 'success',
          })
          break

        case 'stream_event':
          if (data.event?.type === 'message_start') {
            entries.push({
              type: 'api_call',
              timestamp: evt.timestamp,
              label: 'API call start',
              detail: 'message_start received',
            })
          }
          if (data.event?.type === 'content_block_start') {
            const block = data.event.content_block
            if (block?.type === 'tool_use') {
              entries.push({
                type: 'tool',
                timestamp: evt.timestamp,
                label: `Tool: ${block.name || '?'}`,
                detail: block.id ? `id: ${block.id.slice(0, 12)}` : '',
              })
            } else if (block?.type === 'thinking') {
              entries.push({
                type: 'api_call',
                timestamp: evt.timestamp,
                label: 'Thinking start',
                detail: '',
              })
            }
          }
          break

        case 'assistant':
          entries.push({
            type: 'api_call',
            timestamp: evt.timestamp,
            label: 'Assistant message',
            detail: `${data.message?.content?.length || 0} blocks`,
            status: 'success',
          })
          break

        case 'api_retry':
          entries.push({
            type: 'retry',
            timestamp: evt.timestamp,
            label: `Retry ${data.attempt}/${data.max_retries}`,
            detail: data.error || '',
            duration: data.retry_delay_ms,
            status: 'error',
          })
          break

        case 'tool_use_summary':
          entries.push({
            type: 'tool',
            timestamp: evt.timestamp,
            label: 'Tool summary',
            detail: data.summary?.slice(0, 80) || '',
            status: 'success',
          })
          break

        case 'result':
          entries.push({
            type: 'result',
            timestamp: evt.timestamp,
            label: data.is_error ? 'Result (error)' : 'Result (success)',
            detail: `${data.num_turns} turns, $${data.total_cost_usd?.toFixed(4) || '0'}`,
            duration: data.duration_ms,
            status: data.is_error ? 'error' : 'success',
          })
          break
      }
    } catch {
      // ignore parse errors
    }
  }

  return entries
}
