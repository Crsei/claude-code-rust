import React from 'react'
import { c } from '../../theme.js'
import type { CustomStatusLineState } from '../../store/app-store.js'

/**
 * Renders the user-configured custom statusline. Mirrors the sample
 * tree's `StatusLine.tsx` output slot
 * (`ui/examples/upstream-patterns/src/components/StatusLine.tsx`): a
 * thin row above the built-in statusline showing the stdout of the
 * user's configured statusline command.
 *
 * Called only after `shouldRenderCustomStatusLine` has approved the
 * snapshot, so this component doesn't re-check the presence /
 * error flags — it just renders.
 */

type Props = {
  snapshot: CustomStatusLineState
}

export function CustomStatusLine({ snapshot }: Props) {
  return (
    <box flexDirection="column" paddingX={1}>
      {snapshot.lines.map((line, index) => (
        <text key={index} fg={c.dim} selectable>
          {line}
        </text>
      ))}
    </box>
  )
}
