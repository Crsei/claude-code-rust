import React from 'react'
import { c } from '../../theme.js'

/**
 * One-line summary of what a connected MCP server exposes (tools,
 * resources, prompts). Drops the upstream `<Byline>` wrapper — OpenTUI
 * has no equivalent primitive — and uses a " · " join instead.
 */

type Props = {
  serverToolsCount: number
  serverResourcesCount: number
  serverPromptsCount?: number
}

export function CapabilitiesSection({
  serverToolsCount,
  serverResourcesCount,
  serverPromptsCount = 0,
}: Props) {
  const capabilities: string[] = []
  if (serverToolsCount > 0) capabilities.push('tools')
  if (serverResourcesCount > 0) capabilities.push('resources')
  if (serverPromptsCount > 0) capabilities.push('prompts')

  return (
    <box>
      <text>
        <strong>Capabilities: </strong>
        {capabilities.length > 0 ? (
          <span>{capabilities.join(' · ')}</span>
        ) : (
          <span fg={c.dim}>none</span>
        )}
      </text>
    </box>
  )
}
