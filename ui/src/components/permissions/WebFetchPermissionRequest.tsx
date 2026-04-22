import React from 'react'
import { c } from '../../theme.js'

/**
 * Body for the permission dialog when the requested tool wants to
 * fetch a URL (`WebFetch`, `WebSearch`). Mirrors the sample tree's
 * `WebFetchPermissionRequest` but strips the analytics + domain-rule
 * breakdown; showing the URL itself is the critical context.
 */

type Props = {
  url: string
}

export function WebFetchPermissionRequest({ url }: Props) {
  return (
    <box flexDirection="column">
      <text fg={c.dim}>URL</text>
      <box border={['left']} borderColor={c.warning} paddingLeft={1} paddingRight={1}>
        <text selectable>
          <strong>{url}</strong>
        </text>
      </box>
    </box>
  )
}
