import type { ResultEvent } from '@/lib/types'
import { CheckCircle2, XCircle, Clock, Zap, Coins } from 'lucide-react'

interface ResultBannerProps {
  result: ResultEvent
}

/**
 * Shows a compact result summary at the end of a response.
 * Displays duration, turns, cost, and error status.
 */
export function ResultBanner({ result }: ResultBannerProps) {
  const isError = result.is_error

  return (
    <div className={`flex items-center gap-3 rounded-lg px-3 py-1.5 text-[10px] ${
      isError
        ? 'bg-red-950/20 border border-red-500/20 text-red-300'
        : 'bg-muted/30 border border-border/30 text-muted-foreground'
    }`}>
      {isError ? (
        <XCircle className="h-3.5 w-3.5 text-red-400 shrink-0" />
      ) : (
        <CheckCircle2 className="h-3.5 w-3.5 text-green-400/60 shrink-0" />
      )}

      <span className="flex items-center gap-1">
        <Clock className="h-3 w-3" />
        {formatDuration(result.duration_ms)}
      </span>

      {result.duration_api_ms > 0 && (
        <span className="flex items-center gap-1">
          <Zap className="h-3 w-3" />
          API {formatDuration(result.duration_api_ms)}
        </span>
      )}

      {result.num_turns > 0 && (
        <span>{result.num_turns} turn{result.num_turns !== 1 ? 's' : ''}</span>
      )}

      <span className="flex items-center gap-1 font-mono">
        <Coins className="h-3 w-3" />
        ${result.total_cost_usd.toFixed(4)}
      </span>

      {result.usage.total_input_tokens > 0 && (
        <span className="text-blue-400/60">
          {result.usage.total_input_tokens.toLocaleString()}in
        </span>
      )}
      {result.usage.total_output_tokens > 0 && (
        <span className="text-green-400/60">
          {result.usage.total_output_tokens.toLocaleString()}out
        </span>
      )}

      {result.errors.length > 0 && (
        <span className="text-red-400">
          {result.errors.length} error{result.errors.length > 1 ? 's' : ''}
        </span>
      )}
    </div>
  )
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}
