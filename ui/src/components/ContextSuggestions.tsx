import React from 'react'
import { c } from '../theme.js'
import { formatTokens } from '../utils.js'

/**
 * Inline context-health recommendations ("try /compact", "keep fewer
 * tabs open", etc).
 *
 * OpenTUI-native port of the upstream `ContextSuggestions`
 * (`ui/examples/upstream-patterns/src/components/ContextSuggestions.tsx`).
 * Upstream wrapped Ink's `StatusIcon` and imported the `figures` npm
 * module. The Lite port inlines tiny severity glyphs and renders with
 * OpenTUI's `<text>`/`<box>` elements instead.
 */

export type ContextSuggestionSeverity = 'info' | 'warning' | 'error' | 'success'

export type ContextSuggestion = {
  title: string
  detail: string
  severity: ContextSuggestionSeverity
  savingsTokens?: number
}

const SEVERITY_GLYPH: Record<ContextSuggestionSeverity, string> = {
  info: '\u2139',
  warning: '\u26A0',
  error: '\u2716',
  success: '\u2713',
}

const SEVERITY_COLOR: Record<ContextSuggestionSeverity, string> = {
  info: c.info,
  warning: c.warning,
  error: c.error,
  success: c.success,
}

type Props = {
  suggestions: ContextSuggestion[]
}

export function ContextSuggestions({ suggestions }: Props) {
  if (suggestions.length === 0) return null
  return (
    <box flexDirection="column" marginTop={1}>
      <text>
        <strong>Suggestions</strong>
      </text>
      {suggestions.map((s, i) => {
        const glyph = SEVERITY_GLYPH[s.severity] ?? SEVERITY_GLYPH.info
        const glyphColor = SEVERITY_COLOR[s.severity] ?? c.info
        return (
          <box
            key={`${s.title}-${i}`}
            flexDirection="column"
            marginTop={i === 0 ? 0 : 1}
          >
            <text>
              <span fg={glyphColor}>{glyph} </span>
              <strong>{s.title}</strong>
              {s.savingsTokens !== undefined && s.savingsTokens > 0 && (
                <span fg={c.dim}>
                  {` \u2192 save ~${formatTokens(s.savingsTokens)}`}
                </span>
              )}
            </text>
            <box marginLeft={2}>
              <text fg={c.dim}>{s.detail}</text>
            </box>
          </box>
        )
      })}
    </box>
  )
}
