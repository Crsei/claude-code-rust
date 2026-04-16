interface BashOutputProps {
  content: string
  isError?: boolean
}

/**
 * Terminal-style output for Bash tool results.
 * Renders in a dark terminal card with monospace font.
 */
export function BashOutput({ content, isError }: BashOutputProps) {
  if (!content) {
    return (
      <div className="rounded bg-zinc-900 px-3 py-2 text-[11px] font-mono text-zinc-500 italic">
        (no output)
      </div>
    )
  }

  return (
    <div className={`rounded bg-zinc-900 px-3 py-2 max-h-80 overflow-auto ${isError ? 'border-l-2 border-red-500' : ''}`}>
      <pre className={`text-[11px] font-mono whitespace-pre-wrap break-all ${isError ? 'text-red-300' : 'text-green-300'}`}>
        {content}
      </pre>
    </div>
  )
}
