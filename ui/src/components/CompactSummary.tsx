import React from 'react'
import { c } from '../theme.js'
import type { ViewMode } from '../keybindings.js'
import { ConfigurableShortcutHint } from './ConfigurableShortcutHint.js'

/**
 * Summary row rendered in place of a collapsed "compact" message.
 *
 * OpenTUI-native port of the upstream `CompactSummary`
 * (`ui/examples/upstream-patterns/src/components/CompactSummary.tsx`).
 * Upstream consumed `NormalizedUserMessage` / `screen` types that live
 * in the Ink transcript pipeline. The Lite port accepts a flat
 * `CompactSummaryData` shape so the message pipeline — which in this
 * tree is `message-model.ts` — can project the same fields without
 * pulling the upstream message normalizer in.
 */

const BLACK_CIRCLE = '\u25CF'

export type CompactSummaryMetadata = {
  messagesSummarized?: number
  direction?: 'up_to' | 'from_here' | string
  userContext?: string
}

export type CompactSummaryData = {
  /** Rendered text form of the summary (shown in transcript mode). */
  text: string
  /** Present only for explicit "Summarize from here" / "up to" commands. */
  metadata?: CompactSummaryMetadata
}

type Props = {
  summary: CompactSummaryData
  viewMode: ViewMode
}

export function CompactSummary({ summary, viewMode }: Props) {
  const isTranscriptMode = viewMode === 'transcript'
  const { metadata, text } = summary

  if (metadata) {
    return (
      <box flexDirection="column" marginTop={1}>
        <box flexDirection="row">
          <box minWidth={2}>
            <text>{BLACK_CIRCLE}</text>
          </box>
          <box flexDirection="column">
            <text>
              <strong>Summarized conversation</strong>
            </text>
            {!isTranscriptMode ? (
              <box flexDirection="column" marginLeft={0}>
                <text fg={c.dim}>
                  Summarized {metadata.messagesSummarized} messages
                  {metadata.direction === 'up_to'
                    ? ' up to this point'
                    : ' from this point'}
                </text>
                {metadata.userContext && (
                  <text fg={c.dim}>
                    {'Context: \u201C'}
                    {metadata.userContext}
                    {'\u201D'}
                  </text>
                )}
                <text>
                  <ConfigurableShortcutHint
                    action="app:toggleTranscript"
                    context="Global"
                    fallback="ctrl+o"
                    description="expand history"
                    parens
                  />
                </text>
              </box>
            ) : (
              <box marginLeft={0}>
                <text>{text}</text>
              </box>
            )}
          </box>
        </box>
      </box>
    )
  }

  return (
    <box flexDirection="column" marginTop={1}>
      <box flexDirection="row">
        <box minWidth={2}>
          <text>{BLACK_CIRCLE}</text>
        </box>
        <box flexDirection="column">
          <text>
            <strong>Compact summary</strong>
            {!isTranscriptMode && (
              <>
                {' '}
                <ConfigurableShortcutHint
                  action="app:toggleTranscript"
                  context="Global"
                  fallback="ctrl+o"
                  description="expand"
                  parens
                />
              </>
            )}
          </text>
        </box>
      </box>
      {isTranscriptMode && (
        <box marginLeft={2}>
          <text>{text}</text>
        </box>
      )}
    </box>
  )
}
