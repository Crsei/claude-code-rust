import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'

interface MarkdownContentProps {
  content: string
}

/**
 * Renders markdown text with GFM support (tables, strikethrough, etc.)
 * Styled to match the dark theme.
 */
export function MarkdownContent({ content }: MarkdownContentProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        // Code blocks
        pre: ({ children }) => (
          <pre className="my-2 overflow-x-auto rounded-md bg-zinc-900 px-3 py-2 text-[12px]">
            {children}
          </pre>
        ),
        code: ({ className, children, ...props }) => {
          const isBlock = className?.startsWith('language-')
          if (isBlock) {
            return (
              <code className="font-mono text-green-300" {...props}>
                {children}
              </code>
            )
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
