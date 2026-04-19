import { useEffect, useRef, useState, useCallback } from 'react'
import mermaid from 'mermaid'
import { Copy, Check, AlertTriangle } from 'lucide-react'

interface MermaidDiagramProps {
  code: string
}

let mermaidInitialized = false
function ensureMermaidInit() {
  if (mermaidInitialized) return
  mermaid.initialize({
    startOnLoad: false,
    theme: 'dark',
    securityLevel: 'strict',
    fontFamily: 'inherit',
    // Pick a smaller default font that matches the surrounding UI size.
    fontSize: 13,
  })
  mermaidInitialized = true
}

let diagramCounter = 0

/**
 * Renders a fenced ```mermaid block as an SVG diagram. Runs mermaid.render
 * once per source change and shows the raw source if parsing fails so the
 * user can still see and debug the diagram body.
 */
export function MermaidDiagram({ code }: MermaidDiagramProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    let cancelled = false
    ensureMermaidInit()
    const id = `mermaid-${++diagramCounter}`
    mermaid
      .render(id, code)
      .then(({ svg }) => {
        if (cancelled) return
        if (containerRef.current) {
          containerRef.current.innerHTML = svg
        }
        setError(null)
      })
      .catch((err: unknown) => {
        if (cancelled) return
        setError(err instanceof Error ? err.message : String(err))
        // Mermaid leaves a failed-render element glued to document.body — the
        // div (id="dmermaid-…") is how it surfaces syntax errors during render.
        // Clean it up so the page doesn't accumulate orphaned error nodes.
        try {
          const leftover = document.getElementById(`d${id}`)
          leftover?.remove()
        } catch { /* defensive */ }
      })
    return () => {
      cancelled = true
    }
  }, [code])

  const handleCopy = useCallback(() => {
    navigator.clipboard
      .writeText(code)
      .then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      })
      .catch(() => { /* ignore */ })
  }, [code])

  return (
    <div className="group relative my-2 overflow-hidden rounded-md border border-border/60 bg-zinc-900">
      <div className="flex items-center justify-between border-b border-black/20 bg-zinc-800 px-3 py-1">
        <span className="text-[10px] font-mono uppercase text-zinc-400">
          mermaid
        </span>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-1 text-[10px] text-zinc-400 hover:text-zinc-200 transition-colors"
          title="Copy source"
        >
          {copied ? (
            <>
              <Check className="h-3 w-3 text-green-400" />
              <span className="text-green-400">Copied</span>
            </>
          ) : (
            <>
              <Copy className="h-3 w-3" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      {error ? (
        <div className="space-y-2 p-3">
          <div className="flex items-center gap-2 text-xs text-amber-400">
            <AlertTriangle className="h-3.5 w-3.5" />
            Mermaid couldn&apos;t render this diagram.
          </div>
          <div className="text-[10px] font-mono text-red-300/80 whitespace-pre-wrap">
            {error}
          </div>
          <pre className="overflow-auto text-[11px] font-mono text-zinc-300 whitespace-pre">
            {code}
          </pre>
        </div>
      ) : (
        <div
          ref={containerRef}
          className="flex justify-center overflow-auto p-3 [&>svg]:max-w-full"
        />
      )}
    </div>
  )
}
