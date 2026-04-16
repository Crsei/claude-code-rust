import { useState } from 'react'
import type { ChatMessage } from '@/lib/types'
import { User, Bot, AlertCircle, ChevronDown, ChevronRight } from 'lucide-react'

interface MessageInspectorProps {
  messages: ChatMessage[]
}

const ROLE_ICONS = {
  user: User,
  assistant: Bot,
  system: AlertCircle,
}

const ROLE_COLORS = {
  user: 'text-blue-400 bg-blue-500/10',
  assistant: 'text-green-400 bg-green-500/10',
  system: 'text-yellow-400 bg-yellow-500/10',
}

export function MessageInspector({ messages }: MessageInspectorProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null)

  return (
    <div className="flex-1 overflow-y-auto p-2 space-y-1">
      {messages.length === 0 && (
        <div className="text-xs text-muted-foreground italic py-4 text-center">
          No messages yet
        </div>
      )}
      {messages.map((msg, i) => {
        const isExpanded = expandedId === msg.id
        const Icon = ROLE_ICONS[msg.role] || AlertCircle
        const colorClass = ROLE_COLORS[msg.role] || 'text-foreground bg-muted'
        const hasBlocks = msg.contentBlocks && msg.contentBlocks.length > 0
        const hasTools = msg.toolCalls && msg.toolCalls.length > 0

        return (
          <button
            key={msg.id}
            onClick={() => setExpandedId(isExpanded ? null : msg.id)}
            className="w-full text-left rounded border border-border/30 bg-card overflow-hidden hover:border-border/60 transition-colors"
          >
            {/* Message header */}
            <div className="flex items-center gap-2 px-2 py-1.5">
              {isExpanded
                ? <ChevronDown className="h-3 w-3 text-muted-foreground shrink-0" />
                : <ChevronRight className="h-3 w-3 text-muted-foreground shrink-0" />
              }
              <span className={`flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-medium ${colorClass}`}>
                <Icon className="h-3 w-3" />
                {msg.role}
              </span>
              <span className="text-[10px] text-muted-foreground">
                #{i + 1}
              </span>
              {hasTools && (
                <span className="text-[9px] rounded bg-cyan-500/10 text-cyan-400 px-1 py-0.5">
                  {msg.toolCalls!.length} tool{msg.toolCalls!.length > 1 ? 's' : ''}
                </span>
              )}
              {hasBlocks && (
                <span className="text-[9px] rounded bg-purple-500/10 text-purple-400 px-1 py-0.5">
                  {msg.contentBlocks!.length} blocks
                </span>
              )}
              <span className="ml-auto text-[9px] text-muted-foreground/50">
                {new Date(msg.timestamp).toLocaleTimeString()}
              </span>
            </div>

            {/* Collapsed preview */}
            {!isExpanded && (
              <div className="px-2 pb-1.5 text-[10px] text-foreground/50 truncate font-mono">
                {msg.content.slice(0, 120)}
              </div>
            )}

            {/* Expanded details */}
            {isExpanded && (
              <div className="border-t border-border/30 px-2 py-1.5 space-y-2">
                {/* Content */}
                <div>
                  <div className="text-[9px] uppercase tracking-wide text-muted-foreground mb-0.5">Content</div>
                  <pre className="text-[10px] font-mono text-foreground/70 whitespace-pre-wrap max-h-40 overflow-y-auto bg-muted/30 rounded p-1.5">
                    {msg.content || '(empty)'}
                  </pre>
                </div>

                {/* Content blocks */}
                {hasBlocks && (
                  <div>
                    <div className="text-[9px] uppercase tracking-wide text-muted-foreground mb-0.5">
                      Content Blocks ({msg.contentBlocks!.length})
                    </div>
                    {msg.contentBlocks!.map((block, bi) => (
                      <div key={bi} className="rounded bg-muted/30 p-1.5 mb-0.5 text-[10px] font-mono">
                        <span className="text-primary font-medium">{block.type}</span>
                        {block.name && <span className="text-cyan-400 ml-1">{block.name}</span>}
                        {block.id && <span className="text-muted-foreground ml-1">id:{block.id.slice(0, 8)}</span>}
                        {block.text && (
                          <pre className="text-foreground/50 mt-0.5 whitespace-pre-wrap max-h-20 overflow-y-auto">
                            {block.text.slice(0, 200)}{block.text.length > 200 ? '...' : ''}
                          </pre>
                        )}
                        {block.input && (
                          <pre className="text-foreground/50 mt-0.5 whitespace-pre-wrap max-h-20 overflow-y-auto">
                            {JSON.stringify(block.input, null, 1).slice(0, 200)}
                          </pre>
                        )}
                      </div>
                    ))}
                  </div>
                )}

                {/* Usage */}
                {msg.usage && (
                  <div className="flex gap-3 text-[9px] text-muted-foreground">
                    <span>in: {msg.usage.input_tokens}</span>
                    <span>out: {msg.usage.output_tokens}</span>
                    {msg.costUsd != null && <span>${msg.costUsd.toFixed(4)}</span>}
                  </div>
                )}

                {/* Meta */}
                <div className="flex gap-3 text-[9px] text-muted-foreground/50">
                  <span>id: {msg.id.slice(0, 8)}</span>
                </div>
              </div>
            )}
          </button>
        )
      })}
    </div>
  )
}
