import React from 'react'
import { useAppState } from '../store/app-store.js'
import type {
  LspServerInfo,
  McpServerStatusInfo,
  PluginInfo,
  SkillInfo,
} from '../ipc/protocol.js'
import { LspServerCard, McpServerCard, PluginRow } from './panels/index.js'

/**
 * Overview of the currently-connected subsystems (MCP, LSP, plugins,
 * skills). Each subsystem-specific card now lives under
 * `./panels/`, and this component stays responsible for outer chrome
 * and section headers.
 */

function LspSection({ servers }: { servers: LspServerInfo[] }) {
  if (servers.length === 0) return null
  return (
    <box flexDirection="column">
      <text><span fg="#89B4FA">LSP</span></text>
      {servers.map(s => (
        <LspServerCard key={s.language_id} server={s} />
      ))}
    </box>
  )
}

function McpSection({ servers }: { servers: McpServerStatusInfo[] }) {
  if (servers.length === 0) return null
  return (
    <box flexDirection="column">
      <text><span fg="#CBA6F7">MCP</span></text>
      {servers.map(s => (
        <McpServerCard key={s.name} server={s} />
      ))}
    </box>
  )
}

function PluginSection({ plugins }: { plugins: PluginInfo[] }) {
  if (plugins.length === 0) return null
  return (
    <box flexDirection="column">
      <text><span fg="#FAB387">Plugins</span></text>
      {plugins.map(p => (
        <PluginRow key={p.id} plugin={p} />
      ))}
    </box>
  )
}

function SkillSection({ skills }: { skills: SkillInfo[] }) {
  if (skills.length === 0) return null
  const invocable = skills.filter(s => s.user_invocable)
  return (
    <box flexDirection="column">
      <text>
        <span fg="#F9E2AF">Skills</span>
        <span fg="#6C7086"> ({skills.length} total, {invocable.length} invocable)</span>
      </text>
    </box>
  )
}

export function SubsystemStatus() {
  const { subsystems } = useAppState()
  const { lsp, mcp, plugins, skills } = subsystems

  const totalItems = lsp.length + mcp.length + plugins.length + skills.length
  if (totalItems === 0) return null

  return (
    <box
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor="#45475A"
      paddingX={1}
      title="Subsystems"
      titleAlignment="left"
      gap={0}
    >
      <LspSection servers={lsp} />
      <McpSection servers={mcp} />
      <PluginSection plugins={plugins} />
      <SkillSection skills={skills} />
    </box>
  )
}
