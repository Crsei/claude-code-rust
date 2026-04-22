import React from 'react'
import { c } from '../../theme.js'

/**
 * Body shown inside the permission dialog frame when the requested
 * tool is a shell command (`Bash`, `PowerShell`). Mirrors the
 * `BashPermissionRequest` group in the sample tree
 * (`ui/examples/upstream-patterns/src/components/permissions/BashPermissionRequest/`)
 * by showing the command prominently; no analytics or rule preview.
 */

type Props = {
  command: string
}

export function BashPermissionRequest({ command }: Props) {
  return (
    <box flexDirection="column">
      <text fg={c.dim}>Command</text>
      <box
        border={['left']}
        borderColor={c.warning}
        paddingLeft={1}
        paddingRight={1}
      >
        <text selectable>
          <strong>{command}</strong>
        </text>
      </box>
    </box>
  )
}
