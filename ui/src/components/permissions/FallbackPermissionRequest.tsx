import React from 'react'
import { c } from '../../theme.js'

/**
 * Catch-all body for permission requests that don't match one of the
 * categorized variants. Mirrors the sample tree's
 * `FallbackPermissionRequest`
 * (`ui/examples/upstream-patterns/src/components/permissions/FallbackPermissionRequest.tsx`)
 * while staying on OpenTUI primitives.
 */

type Props = {
  command: string
}

export function FallbackPermissionRequest({ command }: Props) {
  return (
    <box flexDirection="column">
      <text fg={c.dim}>Command</text>
      <box
        border={['left']}
        borderColor={c.warning}
        paddingLeft={1}
        paddingRight={1}
      >
        <text selectable>{command || '(no command payload supplied)'}</text>
      </box>
    </box>
  )
}
