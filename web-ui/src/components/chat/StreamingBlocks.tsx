import type { StreamingBlock } from '@/lib/types'
import { ThinkingBlock } from './ThinkingBlock'
import { Terminal, Wrench, Loader2 } from 'lucide-react'

interface StreamingBlocksProps {
  blocks: StreamingBlock[]
}

/**
 * Renders the live streaming blocks during an active response.
 * Replaces the simple StreamingText component for multi-block responses.
 */
export function StreamingBlocks({ blocks }: StreamingBlocksProps) {
  if (blocks.length === 0) return null

  return (
    <div className="flex justify-start">
      <div className="max-w-[85%] space-y-1">
        {blocks.filter(Boolean).map((block) => (
          <StreamingBlockItem key={block.index} block={block} />
        ))}
      </div>
    </div>
  )
}

function StreamingBlockItem({ block }: { block: StreamingBlock }) {
  switch (block.type) {
    case 'thinking':
      return (
        <ThinkingBlock
          thinking={block.content}
          isStreaming={!block.done}
        />
      )

    case 'tool_use':
      return <StreamingToolCall block={block} />

    case 'text':
    default:
      return <StreamingTextBlock block={block} />
  }
}

/** Streaming text block with cursor */
function StreamingTextBlock({ block }: { block: StreamingBlock }) {
  if (!block.content && block.done) return null

  return (
    <div className="rounded-2xl rounded-bl-md bg-secondary px-4 py-2.5 text-sm text-secondary-foreground">
      <div className="whitespace-pre-wrap break-words">
        {block.content}
        {!block.done && (
          <span className="inline-block h-4 w-1.5 animate-pulse bg-foreground/70 ml-0.5" />
        )}
      </div>
    </div>
  )
}

/** Streaming tool call — shows tool name and accumulating input */
function StreamingToolCall({ block }: { block: StreamingBlock }) {
  const toolName = block.toolName || 'tool'
  const isBash = toolName.toLowerCase() === 'bash' || toolName.toLowerCase() === 'execute_bash'

  return (
    <div className="my-1 rounded-lg border border-border/60 bg-card overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-2 text-xs">
        {isBash
          ? <Terminal className="h-3.5 w-3.5 text-muted-foreground" />
          : <Wrench className="h-3.5 w-3.5 text-muted-foreground" />
        }
        <span className="font-mono font-medium text-foreground">{toolName}</span>
        {!block.done && (
          <Loader2 className="h-3 w-3 animate-spin text-blue-400 ml-auto" />
        )}
      </div>
      {block.toolInput && (
        <div className="border-t border-border/40 px-3 py-1.5">
          <pre className="text-[11px] font-mono text-foreground/60 max-h-32 overflow-auto whitespace-pre-wrap">
            {tryPrettyJson(block.toolInput)}
          </pre>
        </div>
      )}
    </div>
  )
}

function tryPrettyJson(s: string): string {
  try {
    return JSON.stringify(JSON.parse(s), null, 2)
  } catch {
    return s
  }
}
