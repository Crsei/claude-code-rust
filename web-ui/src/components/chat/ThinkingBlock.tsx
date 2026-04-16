import { useState } from 'react'
import { Brain, ChevronDown, ChevronRight, ShieldOff } from 'lucide-react'

interface ThinkingBlockProps {
  thinking: string
  isRedacted?: boolean
  isStreaming?: boolean
}

/**
 * Collapsible block that displays model's extended thinking.
 * Starts collapsed by default; shows a preview line.
 */
export function ThinkingBlock({ thinking, isRedacted, isStreaming }: ThinkingBlockProps) {
  const [expanded, setExpanded] = useState(false)

  if (isRedacted) {
    return (
      <div className="my-1.5 flex items-center gap-2 rounded-lg border border-border/40 bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
        <ShieldOff className="h-3.5 w-3.5" />
        <span className="italic">Thinking (redacted)</span>
      </div>
    )
  }

  const preview = thinking.slice(0, 100).replace(/\n/g, ' ')

  return (
    <div className="my-1.5 rounded-lg border border-amber-800/30 bg-amber-950/20 overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs hover:bg-amber-950/30 transition-colors"
      >
        {expanded
          ? <ChevronDown className="h-3 w-3 text-amber-500/70 shrink-0" />
          : <ChevronRight className="h-3 w-3 text-amber-500/70 shrink-0" />
        }
        <Brain className="h-3.5 w-3.5 text-amber-500/70" />
        <span className="font-medium text-amber-400/80">Thinking</span>
        {isStreaming && (
          <span className="inline-block h-2.5 w-1 animate-pulse bg-amber-400/60 ml-1" />
        )}
        {!expanded && (
          <span className="truncate text-amber-400/40 ml-1 font-mono text-[11px]">
            {preview}{thinking.length > 100 ? '...' : ''}
          </span>
        )}
      </button>

      {expanded && (
        <div className="border-t border-amber-800/20 px-3 py-2">
          <pre className="whitespace-pre-wrap text-[11px] font-mono text-amber-200/70 max-h-96 overflow-y-auto">
            {thinking}
            {isStreaming && (
              <span className="inline-block h-3 w-1 animate-pulse bg-amber-400/60 ml-0.5" />
            )}
          </pre>
        </div>
      )}
    </div>
  )
}
