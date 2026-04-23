import React from 'react'
import { systemLevelFromRaw } from '../../adapters/index.js'
import type { SystemLevel } from '../../view-model/types.js'
import { c } from '../../theme.js'
import type { SystemTextRenderItem } from '../../store/message-model.js'
import { CompactBoundaryMessage } from './CompactBoundaryMessage.js'

/**
 * OpenTUI port of the upstream `SystemTextMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/SystemTextMessage.tsx`).
 *
 * Upstream renders a left-gutter bullet (● for non-info levels, nothing for
 * info) + the body text coloured by level. We mirror that layout here and
 * extend with two cc-rust-specific levels:
 * - `question` — Lite-only "ask-user" callout with a left border
 * - `compact_boundary` — the `✻ Conversation compacted` separator
 *
 * The `systemLevelFromRaw` adapter keeps unrecognised levels mapping to
 * `'info'` so the style table always hits.
 */

type Props = {
  item: SystemTextRenderItem
}

type DisplayLevel = SystemLevel | 'question' | 'compact_boundary'

const STYLES: Record<DisplayLevel, { color: string; dim: boolean; showBullet: boolean; bullet: string }> = {
  error:            { color: c.error,   dim: false, showBullet: true,  bullet: '●' },
  warning:          { color: c.warning, dim: false, showBullet: true,  bullet: '●' },
  info:             { color: c.dim,     dim: true,  showBullet: false, bullet: '' },
  success:          { color: c.success, dim: false, showBullet: true,  bullet: '●' },
  debug:            { color: c.dim,     dim: true,  showBullet: true,  bullet: '◌' },
  question:         { color: c.warning, dim: false, showBullet: false, bullet: '?' },
  compact_boundary: { color: c.dim,     dim: true,  showBullet: false, bullet: '' },
}

function resolveLevel(raw: string | undefined): DisplayLevel {
  if (raw === 'question') return 'question'
  if (raw === 'compact_boundary' || raw === 'compact') return 'compact_boundary'
  return systemLevelFromRaw(raw)
}

export function SystemMessage({ item }: Props) {
  const level = resolveLevel(item.level)

  // Compact boundary is a dedicated separator — no bullet, centered label,
  // matches upstream `CompactBoundaryMessage`.
  if (level === 'compact_boundary') {
    return <CompactBoundaryMessage content={item.content} />
  }

  const style = STYLES[level]

  // Question is a prompt-style callout — keep the left border for emphasis.
  if (level === 'question') {
    return (
      <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
        <box border={['left']} borderColor={c.warning} paddingLeft={1} backgroundColor={c.bg}>
          <text selectable fg={style.color} bg={c.bg}>
            [?] {item.content}
          </text>
        </box>
      </box>
    )
  }

  return (
    <box flexDirection="row" paddingX={1} marginBottom={1} width="100%">
      {style.showBullet && (
        <box minWidth={2} flexShrink={0}>
          <text fg={style.color} bg={c.bg}>{style.bullet}</text>
        </box>
      )}
      <box flexDirection="column" width="100%">
        <text selectable fg={style.color} bg={c.bg}>
          {item.content}
        </text>
      </box>
    </box>
  )
}
