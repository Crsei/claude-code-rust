import React from 'react'
import { shortcutLabel } from '../../keybindings.js'
import { c } from '../../theme.js'
import type { ToolGroupRenderItem } from '../../store/message-model.js'
import { useAppState } from '../../store/app-store.js'
import type { ToolStatus } from '../../view-model/types.js'

/**
 * OpenTUI port of upstream `CollapsedReadSearchContent`
 * (`ui/examples/upstream-patterns/src/components/messages/CollapsedReadSearchContent.tsx`)
 * — the collapsed Read/Glob/Grep group row shown in prompt view.
 *
 * Upstream anchors the group with a status dot + one-line summary, followed
 * by a `⎿` continuation line showing the most recent target (file path or
 * search pattern). We reproduce that layout here: latest-item hint comes
 * from `item.previewLines[last]` which `buildRenderItems` already
 * populates.
 *
 * The colour map is keyed by `ToolStatus` from the view-model layer so it
 * stays aligned with the adapter's status classification.
 */

type Props = {
  item: ToolGroupRenderItem
}

interface StatusStyle {
  color: string
  glyph: string
}

const STATUS_STYLES: Record<ToolStatus, StatusStyle> = {
  pending:   { color: c.dim,     glyph: '\u25CC' }, // ◌
  running:   { color: c.info,    glyph: '\u25CF' }, // ●
  success:   { color: c.success, glyph: '\u2713' }, // ✓
  error:     { color: c.error,   glyph: '\u2717' }, // ✗
  cancelled: { color: c.warning, glyph: '\u25A0' }, // ■
}

export function ToolGroupMessage({ item }: Props) {
  const { keybindingConfig } = useAppState()
  const style = STATUS_STYLES[item.status]
  const expandHint = `${shortcutLabel('app:toggleTranscript', { context: 'Global', config: keybindingConfig })} to expand`
  const latestHint = item.previewLines[item.previewLines.length - 1]

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box flexDirection="row" gap={1} width="100%">
        <box minWidth={2} flexShrink={0}>
          <text fg={style.color}>{style.glyph}</text>
        </box>
        <text fg={style.color}>
          <strong>{item.title}</strong>
        </text>
        <text fg={c.dim}>({expandHint})</text>
      </box>
      {latestHint && (
        <box paddingLeft={3} flexDirection="row" width="100%">
          <text fg={c.dim}>{'\u23BF '}</text>
          <text fg={item.status === 'running' ? style.color : c.dim} selectable>
            {latestHint}
          </text>
        </box>
      )}
      {item.hiddenCount > 0 && (
        <box paddingLeft={3} width="100%">
          <text fg={c.dim}>
            +{item.hiddenCount} more tool uses ({expandHint})
          </text>
        </box>
      )}
    </box>
  )
}
