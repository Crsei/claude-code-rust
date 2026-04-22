import React from 'react'
import type { ViewMode } from '../../keybindings.js'
import { c } from '../../theme.js'
import { useAppState } from '../../store/app-store.js'
import type { Usage } from '../../store/app-store.js'
import { formatCost, formatTokens } from '../../utils.js'
import {
  countActiveTeams,
  countConnectedMcp,
  countRunningAgents,
  countRunningLsp,
  cwdShortName,
  statusLineError,
} from './status-line-state.js'

/**
 * Built-in statusline body shown across the top of the live
 * conversation pane. Lite-native counterpart of the sample tree's
 * `BuiltinStatusLine`
 * (`ui/examples/upstream-patterns/src/components/BuiltinStatusLine.tsx`),
 * but derived from the current Lite store rather than a cost-tracker
 * global or rate-limit API.
 *
 * Fields shown:
 * - `cwd` short name
 * - transcript / vim indicators when active
 * - active `model`
 * - live counters: running agents, active teams, connected MCP, running LSP
 * - usage tokens + cost from the store's `Usage`
 * - an error pill when the most recent custom statusline run reported
 *   an error (sourced from the shared `CustomStatusLineState`)
 */

interface Props {
  cwd: string
  model: string
  usage: Usage
  vimMode?: string
  viewMode?: ViewMode
}

export function BuiltinStatusLine({
  cwd,
  model,
  usage,
  vimMode,
  viewMode = 'prompt',
}: Props) {
  const { agentTree, subsystems, teams, customStatusLine } = useAppState()
  const runningAgents = countRunningAgents(agentTree)
  const activeTeams = countActiveTeams(teams)
  const connectedMcp = countConnectedMcp(subsystems)
  const runningLsp = countRunningLsp(subsystems)
  const customError = statusLineError(customStatusLine)
  const dir = cwdShortName(cwd)

  return (
    <box
      flexDirection="row"
      paddingX={1}
      border={['bottom']}
      borderStyle="single"
      borderColor="#45475A"
    >
      <text fg={c.accent}>{dir}</text>
      <text fg={c.dim}> | </text>
      {viewMode === 'transcript' && (
        <>
          <text fg={c.info}>
            <strong>[TRANSCRIPT]</strong>
          </text>
          <text fg={c.dim}> | </text>
        </>
      )}
      {vimMode && (
        <>
          <text>
            <strong>
              <span fg={c.warning}>[{vimMode}]</span>
            </strong>
          </text>
          <text fg={c.dim}> | </text>
        </>
      )}
      <text>{model}</text>
      {runningAgents > 0 && (
        <>
          <text fg={c.dim}> | </text>
          <text fg="#A6E3A1">{runningAgents} agent{runningAgents > 1 ? 's' : ''}</text>
        </>
      )}
      {activeTeams > 0 && (
        <>
          <text fg={c.dim}> | </text>
          <text fg="#CBA6F7">{activeTeams} team{activeTeams > 1 ? 's' : ''}</text>
        </>
      )}
      {(connectedMcp > 0 || runningLsp > 0) && (
        <>
          <text fg={c.dim}> | </text>
          {runningLsp > 0 && <text fg="#89B4FA">LSP:{runningLsp}</text>}
          {runningLsp > 0 && connectedMcp > 0 && <text fg={c.dim}>/</text>}
          {connectedMcp > 0 && <text fg="#CBA6F7">MCP:{connectedMcp}</text>}
        </>
      )}
      {customError && (
        <>
          <text fg={c.dim}> | </text>
          <text fg="#F38BA8">statusline: {customError}</text>
        </>
      )}
      <box flexGrow={1} />
      <text fg={c.dim}>Tokens: </text>
      <text>{formatTokens(usage.inputTokens + usage.outputTokens)}</text>
      <text fg={c.dim}> | Cost: </text>
      <text fg={c.success}>{formatCost(usage.costUsd)}</text>
    </box>
  )
}
