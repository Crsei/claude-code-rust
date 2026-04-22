import React from 'react'
import { shortcutLabel } from '../../keybindings.js'
import { c } from '../../theme.js'
import type { ToolGroupRenderItem } from '../../store/message-model.js'
import { useAppState } from '../../store/app-store.js'
import type { ToolStatus } from '../../view-model/types.js'

/**
 * Lite-native port of the sample tree's `GroupedToolUseContent`
 * (`ui/examples/upstream-patterns/src/components/messages/GroupedToolUseContent.tsx`),
 * re-hosted on top of the Lite render pipeline's `ToolGroupRenderItem`
 * shape.
 *
 * The colour map is keyed by `ToolStatus` from the view-model layer so
 * it stays aligned with the adapter's status classification.
 */

type Props = {
  item: ToolGroupRenderItem
}

const STATUS_COLORS: Record<ToolStatus, string> = {
  pending: c.dim,
  running: c.info,
  success: c.success,
  error: c.error,
  cancelled: c.warning,
}

export function ToolGroupMessage({ item }: Props) {
  const { keybindingConfig } = useAppState()
  const color = STATUS_COLORS[item.status]
  const expandHint = `${shortcutLabel('app:toggleTranscript', { context: 'Global', config: keybindingConfig })} to expand`

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box border={['left']} borderColor={color} paddingLeft={1} flexDirection="column" width="100%">
        <box gap={1} width="100%">
          <text fg={color}>
            <strong>{item.title}</strong>
          </text>
          <text fg={c.dim}>({expandHint})</text>
        </box>
        <box paddingLeft={2} flexDirection="column" width="100%">
          {item.previewLines.map((line, index) => {
            const isLatestRunning = item.status === 'running' && index === item.previewLines.length - 1
            return (
              <text key={`${item.id}:preview:${index}`} fg={isLatestRunning ? color : c.dim} selectable>
                {line}
              </text>
            )
          })}
          {item.hiddenCount > 0 && (
            <text fg={c.dim}>
              +{item.hiddenCount} more tool uses ({expandHint})
            </text>
          )}
        </box>
      </box>
    </box>
  )
}
