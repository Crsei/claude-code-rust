import type { UsageTracking } from '@/lib/types'
import { BarChart3 } from 'lucide-react'

interface UsagePanelProps {
  usage?: UsageTracking
}

export function UsagePanel({ usage }: UsagePanelProps) {
  if (!usage) return null

  const totalTokens = usage.total_input_tokens + usage.total_output_tokens

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2 text-xs font-medium text-muted-foreground uppercase tracking-wide">
        <BarChart3 className="h-3 w-3" />
        Usage
      </div>

      <div className="rounded-lg border border-border/50 bg-muted/20 p-3 space-y-2">
        {/* Cost */}
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted-foreground">Cost</span>
          <span className="text-sm font-mono font-medium text-foreground">
            ${usage.total_cost_usd.toFixed(4)}
          </span>
        </div>

        {/* Token bar */}
        <div className="space-y-1">
          <div className="flex items-center justify-between text-[10px] text-muted-foreground">
            <span>Tokens</span>
            <span>{totalTokens.toLocaleString()}</span>
          </div>
          <div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
            {totalTokens > 0 && (
              <div className="flex h-full">
                <div
                  className="bg-blue-500 h-full"
                  style={{ width: `${(usage.total_input_tokens / totalTokens) * 100}%` }}
                  title={`Input: ${usage.total_input_tokens.toLocaleString()}`}
                />
                <div
                  className="bg-green-500 h-full"
                  style={{ width: `${(usage.total_output_tokens / totalTokens) * 100}%` }}
                  title={`Output: ${usage.total_output_tokens.toLocaleString()}`}
                />
              </div>
            )}
          </div>
          <div className="flex justify-between text-[10px]">
            <span className="text-blue-400">in: {usage.total_input_tokens.toLocaleString()}</span>
            <span className="text-green-400">out: {usage.total_output_tokens.toLocaleString()}</span>
          </div>
        </div>

        {/* Cache info */}
        {(usage.total_cache_read_tokens > 0 || usage.total_cache_creation_tokens > 0) && (
          <div className="flex justify-between text-[10px] text-muted-foreground pt-1 border-t border-border/30">
            <span>Cache read: {usage.total_cache_read_tokens.toLocaleString()}</span>
            <span>Cache write: {usage.total_cache_creation_tokens.toLocaleString()}</span>
          </div>
        )}

        {/* API calls */}
        <div className="flex items-center justify-between text-[10px] text-muted-foreground pt-1 border-t border-border/30">
          <span>API calls</span>
          <span>{usage.api_call_count}</span>
        </div>
      </div>
    </div>
  )
}
