import React, { useRef } from 'react'
import { c } from '../theme.js'
import { defaultSyntaxStyle } from './opentui-syntax.js'

/**
 * OpenTUI port of upstream `Markdown` / `StreamingMarkdown`
 * (`ui/examples/upstream-patterns/src/components/Markdown.tsx`).
 *
 * Upstream walks `marked.lexer(...)` tokens, formats them with
 * `formatToken` into ANSI, and splices a React-rendered `MarkdownTable`
 * between plain-text chunks. OpenTUI ships a `<markdown>` intrinsic that
 * already handles headings, lists, links, inline code, code fences, and
 * paragraphs. To avoid bundling `marked` on the frontend we lean on that
 * primitive for the common case and keep the upstream component's public
 * shape (`children` as the markdown source + optional `dimColor`).
 *
 * The module-level token cache, `stripPromptXMLTags`, and cli-highlight
 * suspense-boundary trickery are skipped here because OpenTUI handles
 * the rendering. If a future need arises (e.g. tables rendered
 * differently, or a ctrl+o "expand" toggle) they can be re-introduced.
 */

type Props = {
  children: string
  /** When true, render the markdown with dim foreground color. */
  dimColor?: boolean
}

function stripPromptXMLTags(input: string): string {
  // Mirror of upstream's `stripPromptXMLTags` — strips any leading
  // `<system-reminder>…</system-reminder>` wrappers so the markdown
  // body renders without the transport envelope. Kept intentionally
  // minimal — the full upstream helper strips several tag families we
  // don't currently surface.
  const CLOSE = '</system-reminder>'
  let text = input.trimStart()
  while (text.startsWith('<system-reminder>')) {
    const end = text.indexOf(CLOSE)
    if (end < 0) break
    text = text.slice(end + CLOSE.length).trimStart()
  }
  return text
}

export function Markdown({ children, dimColor }: Props) {
  const body = stripPromptXMLTags(children)
  if (!body.trim()) return null

  return (
    <box flexDirection="column" width="100%">
      <markdown
        content={body}
        syntaxStyle={defaultSyntaxStyle}
        bg={c.bg}
        {...(dimColor ? { fg: c.dim } : {})}
      />
    </box>
  )
}

/**
 * Streaming variant — upstream splits at the last top-level block
 * boundary so the stable prefix is memoized and never re-parsed. The
 * OpenTUI `<markdown>` element diffs internally per frame, so we just
 * forward the whole content. The monotonically-advancing ref is kept so
 * external callers that rely on the `StreamingMarkdown` identity still
 * receive the same interface.
 */
export function StreamingMarkdown({ children }: { children: string }) {
  const stablePrefixRef = useRef('')
  const stripped = stripPromptXMLTags(children)
  if (!stripped.startsWith(stablePrefixRef.current)) {
    stablePrefixRef.current = ''
  }
  const lastBlockIdx = stripped.lastIndexOf('\n\n')
  if (lastBlockIdx > stablePrefixRef.current.length) {
    stablePrefixRef.current = stripped.slice(0, lastBlockIdx)
  }
  if (!stripped.trim()) return null
  return (
    <box flexDirection="column" width="100%">
      <markdown content={stripped} syntaxStyle={defaultSyntaxStyle} bg={c.bg} />
    </box>
  )
}
