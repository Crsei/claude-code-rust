import React from 'react'
import type { PluginInfo } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import { stateColor } from './state-colors.js'

/**
 * Compact one-line rendering for a single plugin. Promotes the
 * previously inline row in `SubsystemStatus` so every subsystem uses
 * the same `panels/` card pattern. Version is surfaced inline so
 * operators can confirm which build is active at a glance.
 */

type Props = {
  plugin: PluginInfo
}

export function PluginRow({ plugin }: Props) {
  const color = stateColor(plugin.status)
  const toolsLabel =
    plugin.contributed_tools.length > 0
      ? ` · ${plugin.contributed_tools.length} tool${plugin.contributed_tools.length === 1 ? '' : 's'}`
      : ''
  const skillsLabel =
    plugin.contributed_skills.length > 0
      ? ` · ${plugin.contributed_skills.length} skill${plugin.contributed_skills.length === 1 ? '' : 's'}`
      : ''
  const version = plugin.version ? ` v${plugin.version}` : ''

  return (
    <box flexDirection="column">
      <text>
        {'  '}
        <span fg={color}>{plugin.status}</span>
        {' '}
        <strong><span fg="#CDD6F4">{plugin.name}</span></strong>
        <span fg={c.dim}>{version}{toolsLabel}{skillsLabel}</span>
      </text>
      {plugin.error && (
        <text>
          {'    '}
          <span fg="#F38BA8">{plugin.error}</span>
        </text>
      )}
    </box>
  )
}
