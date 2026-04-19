import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { CodeBlock } from './CodeBlock'
import { MermaidDiagram } from './MermaidDiagram'

interface MarkdownContentProps {
  content: string
}

/**
 * Renders markdown text with GFM support (tables, strikethrough, task lists).
 *
 * Code blocks flow through CodeBlock, which handles syntax highlighting,
 * diff coloring, long-block folding, and copy. The special ```mermaid
 * language routes to MermaidDiagram so diagrams render as SVG.
 */
export function MarkdownContent({ content }: MarkdownContentProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        // react-markdown renders a fenced ```lang block as <pre><code>…</code></pre>.
        // We override the <pre> slot to swallow react-markdown's default
        // wrapping — CodeBlock supplies its own pre/code pair so it can
        // render a header bar above the code. Inline code (without a parent
        // pre) still runs through the `code` handler below.
        pre: ({ children }) => <>{children}</>,
        code: ({ className, children, ...props }) => {
          const match = /language-(\w+)/.exec(className || '')
          const lang = match ? match[1] : null
          const codeStr = String(children).replace(/\n$/, '')

          // Fenced block
          if (lang || codeStr.includes('\n')) {
            if (lang === 'mermaid') {
              return <MermaidDiagram code={codeStr} />
            }
            return <CodeBlock lang={lang} code={codeStr} />
          }

          // Inline code
          return (
            <code
              className="rounded bg-muted px-1 py-0.5 font-mono text-[12px] text-primary"
              {...props}
            >
              {children}
            </code>
          )
        },
        // Links — open externally with basic safety attrs.
        a: ({ href, children }) => (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="text-blue-400 underline underline-offset-2 hover:text-blue-300 break-all"
          >
            {children}
          </a>
        ),
        // Lists
        ul: ({ children }) => <ul className="my-1 ml-4 list-disc space-y-0.5">{children}</ul>,
        ol: ({ children }) => <ol className="my-1 ml-4 list-decimal space-y-0.5">{children}</ol>,
        li: ({ children }) => <li className="leading-relaxed">{children}</li>,
        // Paragraphs
        p: ({ children }) => <p className="my-1 leading-relaxed">{children}</p>,
        // Bold / emphasis
        strong: ({ children }) => (
          <strong className="font-semibold text-foreground">{children}</strong>
        ),
        em: ({ children }) => <em className="italic text-foreground/90">{children}</em>,
        // Headers
        h1: ({ children }) => <h1 className="text-lg font-bold mt-3 mb-1">{children}</h1>,
        h2: ({ children }) => <h2 className="text-base font-bold mt-2 mb-1">{children}</h2>,
        h3: ({ children }) => <h3 className="text-sm font-bold mt-2 mb-1">{children}</h3>,
        h4: ({ children }) => (
          <h4 className="text-sm font-semibold mt-2 mb-0.5 text-foreground/90">{children}</h4>
        ),
        // Blockquote
        blockquote: ({ children }) => (
          <blockquote className="my-2 border-l-2 border-muted-foreground/40 pl-3 text-muted-foreground italic">
            {children}
          </blockquote>
        ),
        // Table
        table: ({ children }) => (
          <div className="my-2 overflow-x-auto rounded border border-border/60">
            <table className="min-w-full text-[12px]">{children}</table>
          </div>
        ),
        thead: ({ children }) => <thead className="bg-muted/40">{children}</thead>,
        th: ({ children }) => (
          <th className="border-b border-border px-2 py-1 text-left font-medium">{children}</th>
        ),
        td: ({ children }) => (
          <td className="border-b border-border/40 px-2 py-1 align-top">{children}</td>
        ),
        // Horizontal rule
        hr: () => <hr className="my-3 border-border/40" />,
      }}
    >
      {content}
    </ReactMarkdown>
  )
}
