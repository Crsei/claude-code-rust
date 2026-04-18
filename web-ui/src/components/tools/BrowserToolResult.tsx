import { useState } from 'react'
import type { ToolResultInnerBlock } from '@/lib/types'
import type { BrowserCategory } from '@/lib/browser-tools'

interface BrowserToolResultProps {
  toolName: string
  category: BrowserCategory
  content: string | ToolResultInnerBlock[] | undefined
  isError?: boolean
}

/**
 * Renderer for tool_result content coming from a browser MCP tool.
 *
 * - Screenshots / visual results → inline base64 image preview.
 * - Console / network logs → structured list when JSON-parseable, raw text fallback.
 * - Navigation / read / write results → plain text preview with optional JSON pretty-print.
 */
export function BrowserToolResult({ toolName: _toolName, category, content, isError }: BrowserToolResultProps) {
  const text = extractText(content)
  const images = extractImages(content)

  if (isError) {
    return (
      <pre className="rounded bg-red-500/10 border border-red-500/30 px-2 py-1.5 text-[11px] font-mono text-red-200 whitespace-pre-wrap max-h-60 overflow-y-auto">
        {text || '(error — no message)'}
      </pre>
    )
  }

  return (
    <div className="space-y-2">
      {images.length > 0 && (
        <div className="space-y-1">
          {images.map((img, i) => (
            <ImagePreview key={i} media={img.mediaType} data={img.data} />
          ))}
        </div>
      )}

      {text && renderByCategory(category, text)}
    </div>
  )
}

function renderByCategory(category: BrowserCategory, text: string): JSX.Element {
  // Observability → try to render as a structured list (console / network).
  if (category === 'observability') {
    const items = tryParseJsonArray(text)
    if (items && items.length > 0) {
      return <LogList items={items} />
    }
  }
  // Navigation / read / write — JSON prettifier if parseable, else raw.
  const asJson = tryPretty(text)
  if (asJson) {
    return (
      <pre className="overflow-x-auto rounded bg-muted/50 px-2 py-1.5 text-[11px] font-mono text-foreground/80 max-h-60 overflow-y-auto">
        {asJson}
      </pre>
    )
  }
  return (
    <pre className="overflow-x-auto rounded bg-muted/50 px-2 py-1.5 text-[11px] font-mono text-foreground/80 max-h-60 overflow-y-auto whitespace-pre-wrap">
      {text}
    </pre>
  )
}

function ImagePreview({ media, data }: { media: string; data: string }) {
  const [expanded, setExpanded] = useState(false)
  const sizeKB = Math.round((data.length * 3) / 4 / 1024)
  return (
    <div className="rounded border border-border/40 bg-muted/30 overflow-hidden">
      <button
        className="flex w-full items-center justify-between px-2 py-1 text-[10px] uppercase tracking-wide text-muted-foreground hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
      >
        <span>Screenshot · {media} · ~{sizeKB}KB</span>
        <span>{expanded ? 'hide' : 'show'}</span>
      </button>
      {expanded && (
        <img
          src={`data:${media};base64,${data}`}
          alt={`Browser screenshot (${media})`}
          className="block max-h-[60vh] w-full object-contain bg-black/40"
        />
      )}
    </div>
  )
}

function LogList({ items }: { items: any[] }) {
  const shown = items.slice(0, 50)
  return (
    <div className="rounded border border-border/40 bg-muted/30 overflow-hidden">
      <div className="px-2 py-1 text-[10px] uppercase tracking-wide text-muted-foreground border-b border-border/40">
        {items.length} item{items.length === 1 ? '' : 's'}{items.length > 50 ? ` (showing 50)` : ''}
      </div>
      <ul className="divide-y divide-border/30 max-h-60 overflow-y-auto">
        {shown.map((it, i) => (
          <li key={i} className="px-2 py-1 text-[11px] font-mono text-foreground/80">
            {summarizeLogItem(it)}
          </li>
        ))}
      </ul>
    </div>
  )
}

// --- helpers ---

function extractText(content: string | ToolResultInnerBlock[] | undefined): string {
  if (!content) return ''
  if (typeof content === 'string') return content
  return content
    .filter(b => b.type === 'text')
    .map(b => b.text ?? '')
    .join('\n')
}

function extractImages(
  content: string | ToolResultInnerBlock[] | undefined,
): Array<{ mediaType: string; data: string }> {
  if (!content || typeof content === 'string') return []
  return content
    .filter(b => b.type === 'image' && b.source && b.source.data)
    .map(b => ({
      mediaType: b.source!.media_type,
      data: b.source!.data,
    }))
}

function tryParseJsonArray(text: string): any[] | null {
  try {
    const parsed = JSON.parse(text)
    if (Array.isArray(parsed)) return parsed
    // wrapped shape: { messages | requests | logs | items | entries: [...] }
    if (parsed && typeof parsed === 'object') {
      for (const k of ['messages', 'requests', 'logs', 'items', 'entries']) {
        if (Array.isArray(parsed[k])) return parsed[k]
      }
    }
    return null
  } catch {
    return null
  }
}

function tryPretty(text: string): string | null {
  try {
    const parsed = JSON.parse(text)
    if (parsed === null || typeof parsed !== 'object') return null
    return JSON.stringify(parsed, null, 2)
  } catch {
    return null
  }
}

function summarizeLogItem(it: any): string {
  if (typeof it !== 'object' || it === null) return String(it)
  // Console message shape
  if (it.type === 'console' || it.level || it.text) {
    const lvl = it.level || it.type || 'log'
    const txt = it.text || it.message || ''
    return `[${lvl}] ${txt}`
  }
  // Network request shape
  if (it.url) {
    const method = it.method || 'GET'
    const status = it.status ?? it.statusCode ?? ''
    return `${method} ${status} ${it.url}`
  }
  // Fallback to first ≤120 chars of JSON
  const s = JSON.stringify(it)
  return s.length > 120 ? s.slice(0, 120) + '…' : s
}
