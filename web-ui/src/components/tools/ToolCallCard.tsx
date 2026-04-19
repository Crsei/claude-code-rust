import { useState } from 'react'
import { ChevronDown, ChevronRight, Terminal, FileText, Search, Wrench, Check, X, Loader2, Globe } from 'lucide-react'
import type { ContentBlock, ToolResultInnerBlock } from '@/lib/types'
import { BashOutput } from './BashOutput'
import { FileChangeCard } from './FileChangeCard'
import { BrowserToolResult } from './BrowserToolResult'
import {
  isBrowserTool,
  classifyBrowserTool,
  categoryColorClass,
} from '@/lib/browser-tools'

interface ToolCallCardProps {
  block: ContentBlock
  /** Matching tool_result block if available */
  result?: ContentBlock
  /** Is this tool still executing? */
  isRunning?: boolean
}

/** Icon for known tool names */
function toolIcon(name: string) {
  const n = name.toLowerCase()
  if (isBrowserTool(name)) return <Globe className="h-3.5 w-3.5" />
  if (n === 'bash' || n === 'execute_bash') return <Terminal className="h-3.5 w-3.5" />
  if (n.includes('file') || n.includes('read') || n.includes('write') || n.includes('edit'))
    return <FileText className="h-3.5 w-3.5" />
  if (n.includes('grep') || n.includes('glob') || n.includes('search'))
    return <Search className="h-3.5 w-3.5" />
  return <Wrench className="h-3.5 w-3.5" />
}

/** Status badge */
function StatusBadge({ result, isRunning }: { result?: ContentBlock; isRunning?: boolean }) {
  if (isRunning) {
    return (
      <span className="flex items-center gap-1 text-[10px] text-blue-400">
        <Loader2 className="h-3 w-3 animate-spin" />
        running
      </span>
    )
  }
  if (!result) return null
  if (result.is_error) {
    return (
      <span className="flex items-center gap-1 text-[10px] text-red-400">
        <X className="h-3 w-3" />
        error
      </span>
    )
  }
  return (
    <span className="flex items-center gap-1 text-[10px] text-green-400">
      <Check className="h-3 w-3" />
      done
    </span>
  )
}

/** Determine if tool result looks like bash output */
function isBashTool(name?: string): boolean {
  if (!name) return false
  const n = name.toLowerCase()
  return n === 'bash' || n === 'execute_bash' || n === 'powershell'
}

/** Determine if tool result is a file operation */
function isFileTool(name?: string): boolean {
  if (!name) return false
  const n = name.toLowerCase()
  return n.includes('write') || n.includes('edit') || n === 'file_write' || n === 'file_edit'
}

export function ToolCallCard({ block, result, isRunning }: ToolCallCardProps) {
  const [expanded, setExpanded] = useState(false)
  const toolName = block.name || 'unknown_tool'
  const browserCategory = classifyBrowserTool(toolName)

  // Format input for display
  const inputStr = block.input
    ? JSON.stringify(block.input, null, 2)
    : '{}'

  // Truncated input preview (first line)
  const inputPreview = block.input
    ? truncateInput(block.input)
    : ''

  return (
    <div className="my-1.5 overflow-hidden rounded-lg border border-border/60 bg-card">
      {/* Header — always visible */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs hover:bg-muted/50 transition-colors"
      >
        {expanded
          ? <ChevronDown className="h-3 w-3 text-muted-foreground shrink-0" />
          : <ChevronRight className="h-3 w-3 text-muted-foreground shrink-0" />
        }
        <span className="text-muted-foreground">{toolIcon(toolName)}</span>
        <span className="font-mono font-medium text-foreground">{toolName}</span>
        {browserCategory && (
          <span
            className={`shrink-0 rounded-sm border px-1 text-[9px] uppercase tracking-wide ${categoryColorClass(browserCategory)}`}
            title={`Browser MCP · ${browserCategory}`}
          >
            browser·{browserCategory}
          </span>
        )}
        {inputPreview && !expanded && (
          <span className="truncate text-muted-foreground font-mono ml-1">{inputPreview}</span>
        )}
        <span className="ml-auto shrink-0">
          <StatusBadge result={result} isRunning={isRunning} />
        </span>
      </button>

      {/* Expanded body */}
      {expanded && (
        <div className="border-t border-border/40 px-3 py-2 space-y-2">
          {/* Input */}
          <div>
            <div className="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">Input</div>
            <pre className="overflow-x-auto rounded bg-muted/50 px-2 py-1.5 text-[11px] font-mono text-foreground/80 max-h-60 overflow-y-auto">
              {inputStr}
            </pre>
          </div>

          {/* Result */}
          {result && (
            <div>
              <div className="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">
                Output {result.is_error && <span className="text-red-400 ml-1">(error)</span>}
              </div>
              {browserCategory ? (
                <BrowserToolResult
                  toolName={toolName}
                  category={browserCategory}
                  content={result.content}
                  isError={result.is_error}
                />
              ) : isBashTool(toolName) ? (
                <BashOutput content={stringifyContent(result.content)} isError={result.is_error} />
              ) : isFileTool(toolName) ? (
                <FileChangeCard
                  content={stringifyContent(result.content)}
                  toolName={toolName}
                  input={block.input}
                />
              ) : (
                <pre className="overflow-x-auto rounded bg-muted/50 px-2 py-1.5 text-[11px] font-mono text-foreground/80 max-h-60 overflow-y-auto whitespace-pre-wrap">
                  {stringifyContent(result.content) || '(empty)'}
                </pre>
              )}
            </div>
          )}

          {/* Running indicator when no result yet */}
          {isRunning && !result && (
            <div className="flex items-center gap-2 py-1 text-xs text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" />
              Executing...
            </div>
          )}
        </div>
      )}
    </div>
  )
}

/** Flatten a tool_result.content into a string (it can be a string or array). */
function stringifyContent(content: string | ToolResultInnerBlock[] | undefined): string {
  if (!content) return ''
  if (typeof content === 'string') return content
  return content
    .map(b => (b.type === 'text' ? (b.text ?? '') : b.type === 'image' ? '[image]' : ''))
    .join('\n')
}

/** Create a short preview of tool input */
function truncateInput(input: Record<string, unknown>): string {
  // For common tools, show the most relevant field
  const command = input.command || input.cmd
  if (typeof command === 'string') {
    return command.length > 60 ? command.slice(0, 60) + '...' : command
  }
  const filePath = input.file_path || input.path
  if (typeof filePath === 'string') {
    return filePath
  }
  const query = input.query || input.pattern
  if (typeof query === 'string') {
    return query.length > 60 ? query.slice(0, 60) + '...' : query
  }
  // Fallback: stringify keys
  const keys = Object.keys(input)
  if (keys.length === 0) return ''
  return keys.slice(0, 3).join(', ')
}
