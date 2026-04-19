import { useState, useCallback, useMemo, useEffect, useRef } from 'react'
import hljs from 'highlight.js/lib/core'
import bash from 'highlight.js/lib/languages/bash'
import javascript from 'highlight.js/lib/languages/javascript'
import typescript from 'highlight.js/lib/languages/typescript'
import python from 'highlight.js/lib/languages/python'
import rust from 'highlight.js/lib/languages/rust'
import json from 'highlight.js/lib/languages/json'
import yaml from 'highlight.js/lib/languages/yaml'
import xml from 'highlight.js/lib/languages/xml'
import css from 'highlight.js/lib/languages/css'
import markdown from 'highlight.js/lib/languages/markdown'
import go from 'highlight.js/lib/languages/go'
import sql from 'highlight.js/lib/languages/sql'
import diff from 'highlight.js/lib/languages/diff'
import 'highlight.js/styles/github-dark.css'
import { Copy, Check, ChevronDown, ChevronUp } from 'lucide-react'

// Only register a curated set — shipping every language would bloat the
// web-ui bundle by several MB. Cover the ones that show up most in cc-rust
// transcripts. Aliases registered so fenced ```py / ```ts still work.
hljs.registerLanguage('bash', bash)
hljs.registerLanguage('shell', bash)
hljs.registerLanguage('sh', bash)
hljs.registerLanguage('javascript', javascript)
hljs.registerLanguage('js', javascript)
hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('ts', typescript)
hljs.registerLanguage('tsx', typescript)
hljs.registerLanguage('jsx', javascript)
hljs.registerLanguage('python', python)
hljs.registerLanguage('py', python)
hljs.registerLanguage('rust', rust)
hljs.registerLanguage('rs', rust)
hljs.registerLanguage('json', json)
hljs.registerLanguage('yaml', yaml)
hljs.registerLanguage('yml', yaml)
hljs.registerLanguage('xml', xml)
hljs.registerLanguage('html', xml)
hljs.registerLanguage('css', css)
hljs.registerLanguage('markdown', markdown)
hljs.registerLanguage('md', markdown)
hljs.registerLanguage('go', go)
hljs.registerLanguage('sql', sql)
hljs.registerLanguage('diff', diff)
hljs.registerLanguage('patch', diff)

interface CodeBlockProps {
  lang: string | null
  code: string
}

/** Threshold above which the code block renders with a collapsed height. */
const FOLD_LINES = 30

/**
 * Code block renderer — syntax highlighting via highlight.js, diff-aware
 * line coloring for ```diff / ```patch, copy button, and a fold-for-long
 * affordance.
 */
export function CodeBlock({ lang, code }: CodeBlockProps) {
  const [copied, setCopied] = useState(false)
  const [expanded, setExpanded] = useState(false)

  const lineCount = useMemo(() => code.split('\n').length, [code])
  const foldable = lineCount > FOLD_LINES
  const isDiff = lang === 'diff' || lang === 'patch' || looksLikeDiff(code)

  const handleCopy = useCallback(() => {
    navigator.clipboard
      .writeText(code)
      .then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      })
      .catch(() => { /* clipboard blocked — user can still select */ })
  }, [code])

  // Render body: for diff, we split per-line and color each line by
  // its leading character. For everything else, highlight.js.
  const body = isDiff ? (
    <DiffBody code={code} />
  ) : (
    <HighlightedBody lang={lang} code={code} />
  )

  return (
    <div className="group relative my-2 overflow-hidden rounded-md border border-border/60 bg-zinc-900">
      {/* Header: language badge + copy */}
      <div className="flex items-center justify-between border-b border-black/20 bg-zinc-800 px-3 py-1">
        <span className="text-[10px] font-mono uppercase text-zinc-400">
          {lang || (isDiff ? 'diff' : 'text')}
          {foldable && (
            <span className="ml-2 text-zinc-500 normal-case">· {lineCount} lines</span>
          )}
        </span>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-1 text-[10px] text-zinc-400 hover:text-zinc-200 transition-colors"
          title="Copy code"
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

      {/* Body */}
      <div
        className={`relative overflow-auto ${
          foldable && !expanded ? 'max-h-80' : ''
        }`}
      >
        {body}
        {foldable && !expanded && (
          <div className="pointer-events-none absolute bottom-0 left-0 right-0 h-14 bg-gradient-to-t from-zinc-900 to-transparent" />
        )}
      </div>

      {foldable && (
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="flex w-full items-center justify-center gap-1 border-t border-black/20 bg-zinc-800/60 py-1 text-[10px] text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
        >
          {expanded ? (
            <>
              <ChevronUp className="h-3 w-3" />
              Collapse
            </>
          ) : (
            <>
              <ChevronDown className="h-3 w-3" />
              Show all {lineCount} lines
            </>
          )}
        </button>
      )}
    </div>
  )
}

/** highlight.js rendered body. Falls back to plain text on unknown languages. */
function HighlightedBody({ lang, code }: { lang: string | null; code: string }) {
  const ref = useRef<HTMLElement>(null)

  useEffect(() => {
    if (!ref.current) return
    // Reset the "highlighted" marker hljs writes — otherwise re-renders
    // (e.g. when the fold toggles) silently skip highlighting.
    ref.current.removeAttribute('data-highlighted')
    try {
      if (lang && hljs.getLanguage(lang)) {
        const result = hljs.highlight(code, { language: lang, ignoreIllegals: true })
        ref.current.innerHTML = result.value
      } else {
        const result = hljs.highlightAuto(code)
        ref.current.innerHTML = result.value
      }
    } catch {
      ref.current.textContent = code
    }
  }, [lang, code])

  return (
    <pre className="m-0 bg-transparent px-3 py-2.5 text-[12px] leading-relaxed">
      <code
        ref={ref}
        className={`hljs bg-transparent ${lang ? `language-${lang}` : ''}`}
      >
        {code}
      </code>
    </pre>
  )
}

/**
 * Line-by-line diff rendering. Colors additions/deletions/hunks so diffs
 * from /git, code reviews, or file-edit tool output read at a glance.
 */
function DiffBody({ code }: { code: string }) {
  const lines = code.split('\n')
  return (
    <pre className="m-0 bg-transparent px-0 py-0 text-[12px] leading-relaxed">
      <code className="block font-mono">
        {lines.map((line, i) => {
          const cls = diffLineClass(line)
          return (
            <div key={i} className={`px-3 whitespace-pre ${cls}`}>
              {line || '\u00A0'}
            </div>
          )
        })}
      </code>
    </pre>
  )
}

function diffLineClass(line: string): string {
  if (line.startsWith('+++') || line.startsWith('---')) {
    return 'text-zinc-400 font-semibold'
  }
  if (line.startsWith('@@')) {
    return 'text-sky-400 bg-sky-500/10'
  }
  if (line.startsWith('+')) {
    return 'text-green-300 bg-green-500/10'
  }
  if (line.startsWith('-')) {
    return 'text-red-300 bg-red-500/10'
  }
  return 'text-zinc-300'
}

/** Heuristic — a block is probably a diff if it has unified-diff markers. */
function looksLikeDiff(code: string): boolean {
  const lines = code.split('\n').slice(0, 30)
  let hits = 0
  for (const line of lines) {
    if (line.startsWith('+++') || line.startsWith('---') || line.startsWith('@@')) hits += 2
    else if (/^[+-][^+-]/.test(line)) hits += 1
  }
  return hits >= 4
}
