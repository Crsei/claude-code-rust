import React from 'react'
import { useAppState } from '../store/app-store.js'

// ---------------------------------------------------------------------------
// State color mapping
// ---------------------------------------------------------------------------

const stateColors: Record<string, string> = {
  running: '#A6E3A1',
  connected: '#A6E3A1',
  installed: '#A6E3A1',
  starting: '#F9E2AF',
  connecting: '#F9E2AF',
  stopped: '#6C7086',
  disconnected: '#6C7086',
  disabled: '#6C7086',
  not_installed: '#6C7086',
  error: '#F38BA8',
}

function stateColor(state: string): string {
  return stateColors[state] ?? '#6C7086'
}

// ---------------------------------------------------------------------------
// LSP section
// ---------------------------------------------------------------------------

function LspSection({ servers }: { servers: Array<{ language_id: string; state: string; open_files_count: number; error?: string }> }) {
  if (servers.length === 0) return null
  return (
    <box flexDirection="column">
      <text><span fg="#89B4FA">LSP</span></text>
      {servers.map(s => (
        <text key={s.language_id}>
          {'  '}
          <span fg={stateColor(s.state)}>{s.state}</span>
          {' '}
          <span fg="#CDD6F4">{s.language_id}</span>
          <span fg="#6C7086"> ({s.open_files_count} files)</span>
          {s.error && <span fg="#F38BA8"> {s.error}</span>}
        </text>
      ))}
    </box>
  )
}

// ---------------------------------------------------------------------------
// MCP section
// ---------------------------------------------------------------------------

function McpSection({ servers }: { servers: Array<{ name: string; state: string; transport: string; tools_count: number; resources_count: number; error?: string }> }) {
  if (servers.length === 0) return null
  return (
    <box flexDirection="column">
      <text><span fg="#CBA6F7">MCP</span></text>
      {servers.map(s => (
        <text key={s.name}>
          {'  '}
          <span fg={stateColor(s.state)}>{s.state}</span>
          {' '}
          <span fg="#CDD6F4">{s.name}</span>
          <span fg="#6C7086"> [{s.transport}] {s.tools_count}T/{s.resources_count}R</span>
          {s.error && <span fg="#F38BA8"> {s.error}</span>}
        </text>
      ))}
    </box>
  )
}

// ---------------------------------------------------------------------------
// Plugin section
// ---------------------------------------------------------------------------

function PluginSection({ plugins }: { plugins: Array<{ id: string; name: string; status: string; contributed_tools: string[]; error?: string }> }) {
  if (plugins.length === 0) return null
  return (
    <box flexDirection="column">
      <text><span fg="#FAB387">Plugins</span></text>
      {plugins.map(p => (
        <text key={p.id}>
          {'  '}
          <span fg={stateColor(p.status)}>{p.status}</span>
          {' '}
          <span fg="#CDD6F4">{p.name}</span>
          {p.contributed_tools.length > 0 && (
            <span fg="#6C7086"> ({p.contributed_tools.length} tools)</span>
          )}
          {p.error && <span fg="#F38BA8"> {p.error}</span>}
        </text>
      ))}
    </box>
  )
}

// ---------------------------------------------------------------------------
// Skill section
// ---------------------------------------------------------------------------

function SkillSection({ skills }: { skills: Array<{ name: string; source: string; user_invocable: boolean }> }) {
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

// ---------------------------------------------------------------------------
// Combined panel
// ---------------------------------------------------------------------------

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
