import React from 'react'
import type { AssistantTextRenderItem } from '../../store/message-model.js'
import { c } from '../../theme.js'
import { ThinkingPreview } from './ThinkingPreview.js'

/**
 * OpenTUI port of the upstream `AssistantTextMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/AssistantTextMessage.tsx`).
 *
 * Upstream uses a leading `●` (BLACK_CIRCLE) glyph on the left gutter as a
 * stable visual anchor for assistant turns — mirrors the bullet convention
 * in `SystemTextMessage` / `AssistantToolUseMessage`. The markdown body sits
 * to the right of the gutter so content aligns across turns.
 *
 * Upstream's error-case branches (rate-limit, invalid-api-key, token-revoked,
 * custom-off-switch, etc.) are deliberately not ported here: those errors
 * surface as structured IPC messages in cc-rust and already land on the
 * system-text pipeline via `SystemMessage` instead.
 */
type Props = {
  item: AssistantTextRenderItem
}

export function AssistantTextMessage({ item }: Props) {
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      {item.thinking && <ThinkingPreview content={item.thinking} />}
      <box flexDirection="row" width="100%">
        <box minWidth={2} flexShrink={0}>
          <text fg={c.text} bg={c.bg}>●</text>
        </box>
        <box flexDirection="column" width="100%" selectable backgroundColor={c.bg}>
          <markdown content={item.content} bg={c.bg} />
        </box>
      </box>
    </box>
  )
}
