import React from 'react'
import { systemLevelFromRaw } from '../../adapters/index.js'
import type { SystemLevel } from '../../view-model/types.js'
import { c } from '../../theme.js'
import type { SystemTextRenderItem } from '../../store/message-model.js'

/**
 * Lite-native port of the sample tree's `SystemTextMessage`
 * (`ui/examples/upstream-patterns/src/components/messages/SystemTextMessage.tsx`).
 * Normalizes the free-form `item.level` string through
 * `systemLevelFromRaw` (from the Issue 01 adapter layer) so downstream
 * style picks always see a closed set of `SystemLevel` values.
 *
 * The current Lite shell also shows a `'question'` level that the
 * adapter does not yet model. We continue to treat that as a distinct
 * prompt-style callout to avoid regressing the ask-user flow while the
 * view-model catches up.
 */

type Props = {
  item: SystemTextRenderItem
}

type DisplayLevel = SystemLevel | 'question'

const STYLES: Record<DisplayLevel, { prefix: string; color: string }> = {
  error: { prefix: '[error]', color: c.error },
  warning: { prefix: '[warn]', color: c.warning },
  info: { prefix: '[info]', color: c.text },
  success: { prefix: '[ok]', color: c.success },
  debug: { prefix: '[debug]', color: c.dim },
  question: { prefix: '[?]', color: c.warning },
}

function resolveLevel(raw: string | undefined): DisplayLevel {
  if (raw === 'question') return 'question'
  return systemLevelFromRaw(raw)
}

export function SystemMessage({ item }: Props) {
  const level = resolveLevel(item.level)
  const { prefix, color } = STYLES[level]

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      {level === 'question' ? (
        <box border={['left']} borderColor={c.warning} paddingLeft={1} backgroundColor={c.bg}>
          <text selectable fg={color} bg={c.bg}>
            {prefix} {item.content}
          </text>
        </box>
      ) : (
        <text selectable fg={color} bg={c.bg}>
          {prefix} {item.content}
        </text>
      )}
    </box>
  )
}
