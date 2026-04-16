import { useState, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Copy, Check } from 'lucide-react'

interface MarkdownContentProps {
  content: string
}

/**
 * Renders markdown text with GFM support (tables, strikethrough, etc.)
 * Includes code block copy button and language badge.
 */
export function MarkdownContent({ content }: MarkdownContentProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        // Code blocks with copy button
        pre: ({ children }) => (
          <div className="group relative my-2">
            <pre className="overflow-x-auto rounded-md bg-zinc-900 px-3 py-2.5 text-[12px] leading-relaxed">
              {children}
            </pre>
          </div>
        ),
        code: ({ className, children, ...props }) => {
          const match = /language-(\w+)/.exec(className || '')
          const lang = match ? match[1] : null

          if (lang || className?.startsWith('language-')) {
            return <CodeBlock lang={lang} code={String(children).replace(/\n$/, '')} />
          }

          return (
            <code className="rounded bg-muted px-1 py-0.5 font-mono text-[12px] text-primary" {...props}>
              {children}
            </code>
          )
        },
        // Links
        a: ({ href, children }) => (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="text-blue-400 underline underline-offset-2 hover:text-blue-300"
          >
            {children}
          </a>
        ),
        // Lists
        ul: ({ children }) => <ul className="my-1 ml-4 list-disc space-y-0.5">{children}</ul>,
        ol: ({ children }) => <ol className="my-1 ml-4 list-decimal space-y-0.5">{children}</ol>,
        // Paragraphs
        p: ({ children }) => <p className="my-1">{children}</p>,
        // Bold
        strong: ({ children }) => <strong className="font-semibold text-foreground">{children}</strong>,
        // Headers
        h1: ({ children }) => <h1 className="text-lg font-bold mt-3 mb-1">{children}</h1>,
        h2: ({ children }) => <h2 className="text-base font-bold mt-2 mb-1">{children}</h2>,
        h3: ({ children }) => <h3 className="text-sm font-bold mt-2 mb-1">{children}</h3>,
        // Blockquote
        blockquote: ({ children }) => (
          <blockquote className="my-1 border-l-2 border-muted-foreground/40 pl-3 text-muted-foreground italic">
            {children}
          </blockquote>
        ),
        // Table
        table: ({ children }) => (
          <div className="my-2 overflow-x-auto">
            <table className="min-w-full text-[12px]">{children}</table>
          </div>
        ),
        th: ({ children }) => (
          <th className="border-b border-border px-2 py-1 text-left font-medium">{children}</th>
        ),
        td: ({ children }) => (
          <td className="border-b border-border/40 px-2 py-1">{children}</td>
        ),
        // Horizontal rule
        hr: () => <hr className="my-3 border-border/40" />,
      }}
    >
      {content}
    </ReactMarkdown>
  )
}

/** Code block with language badge and copy button */
function CodeBlock({ lang, code }: { lang: string | null; code: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [code])

  return (
    <>
      {/* Header with language + copy */}
      <div className="flex items-center justify-between rounded-t-md bg-zinc-800 px-3 py-1 -mb-2">
        {lang && (
          <span className="text-[10px] font-mono text-zinc-400 uppercase">{lang}</span>
        )}
        {!lang && <span />}
        <button
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
      <code className="font-mono text-green-300 block">{code}</code>
    </>
  )
}
