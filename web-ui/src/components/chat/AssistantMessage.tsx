import type { ChatMessage, ContentBlock } from '@/lib/types'
import { ToolCallCard } from '@/components/tools/ToolCallCard'
import { ThinkingBlock } from './ThinkingBlock'
import { MarkdownContent } from './MarkdownContent'

interface AssistantMessageProps {
  message: ChatMessage
}

/**
 * Renders a complete assistant message, handling all content block types:
 * - text: rendered as prose
 * - tool_use: rendered as ToolCallCard
 * - tool_result: paired with preceding tool_use
 * - thinking: rendered as collapsible ThinkingBlock
 * - redacted_thinking: rendered as redacted indicator
 */
export function AssistantMessage({ message }: AssistantMessageProps) {
  const blocks = message.contentBlocks
  const hasBlocks = blocks && blocks.length > 0

  return (
    <div className="flex justify-start">
      <div className="max-w-[85%] space-y-1">
        {hasBlocks ? (
          <ContentBlockRenderer blocks={blocks} />
        ) : (
          // Fallback: render plain text content (Phase 1 compat)
          <div className="rounded-2xl rounded-bl-md bg-secondary px-4 py-2.5 text-sm text-secondary-foreground">
            <div className="prose prose-sm prose-invert max-w-none">
              <MarkdownContent content={message.content} />
            </div>
          </div>
        )}

        {/* Usage footer */}
        {message.usage && (
          <div className="flex gap-3 px-1 text-[10px] text-muted-foreground">
            <span>in: {message.usage.input_tokens.toLocaleString()}</span>
            <span>out: {message.usage.output_tokens.toLocaleString()}</span>
            {message.costUsd != null && <span>${message.costUsd.toFixed(4)}</span>}
          </div>
        )}
      </div>
    </div>
  )
}

/**
 * Renders an array of ContentBlocks, pairing tool_use with tool_result.
 */
function ContentBlockRenderer({ blocks }: { blocks: ContentBlock[] }) {
  // Build a map of tool_use_id → tool_result for pairing
  const resultMap = new Map<string, ContentBlock>()
  for (const block of blocks) {
    if (block.type === 'tool_result' && block.tool_use_id) {
      resultMap.set(block.tool_use_id, block)
    }
  }

  // Collect adjacent text blocks into a single group
  const elements: React.ReactNode[] = []
  let textAccum: string[] = []

  const flushText = () => {
    if (textAccum.length > 0) {
      const text = textAccum.join('')
      elements.push(
        <div
          key={`text-${elements.length}`}
          className="rounded-2xl rounded-bl-md bg-secondary px-4 py-2.5 text-sm text-secondary-foreground"
        >
          <div className="prose prose-sm prose-invert max-w-none">
            <MarkdownContent content={text} />
          </div>
        </div>
      )
      textAccum = []
    }
  }

  for (const block of blocks) {
    switch (block.type) {
      case 'text':
        textAccum.push(block.text || '')
        break

      case 'tool_use':
        flushText()
        elements.push(
          <ToolCallCard
            key={block.id || `tool-${elements.length}`}
            block={block}
            result={block.id ? resultMap.get(block.id) : undefined}
          />
        )
        break

      case 'tool_result':
        // Already paired with tool_use above; render standalone only if unpaired
        if (!block.tool_use_id || !blocks.some(b => b.type === 'tool_use' && b.id === block.tool_use_id)) {
          flushText()
          elements.push(
            <div
              key={`result-${elements.length}`}
              className="my-1 rounded border border-border/40 bg-muted/30 px-3 py-2"
            >
              <div className="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">
                Tool Result {block.is_error && <span className="text-red-400">(error)</span>}
              </div>
              <pre className="text-[11px] font-mono text-foreground/80 whitespace-pre-wrap max-h-40 overflow-y-auto">
                {block.content || '(empty)'}
              </pre>
            </div>
          )
        }
        break

      case 'thinking':
        flushText()
        elements.push(
          <ThinkingBlock
            key={`thinking-${elements.length}`}
            thinking={block.thinking || ''}
          />
        )
        break

      case 'redacted_thinking':
        flushText()
        elements.push(
          <ThinkingBlock
            key={`redacted-${elements.length}`}
            thinking=""
            isRedacted
          />
        )
        break

      default:
        // image, unknown — skip for now
        break
    }
  }

  flushText()
  return <>{elements}</>
}
