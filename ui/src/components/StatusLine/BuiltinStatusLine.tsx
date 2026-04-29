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
  const { agentTree, subsystems, teams, customStatusLine, planWorkflow } = useAppState()
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
      backgroundColor={c.bg}
    >
      <text fg={c.accent} bg={c.bg}>{dir}</text>
      <text fg={c.dim} bg={c.bg}> | </text>
      {viewMode === 'transcript' && (
        <>
          <text fg={c.info} bg={c.bg}>
            <strong><span bg={c.bg}>[TRANSCRIPT]</span></strong>
          </text>
          <text fg={c.dim} bg={c.bg}> | </text>
        </>
      )}
      {vimMode && (
        <>
          <text bg={c.bg}>
            <strong>
              <span fg={c.warning} bg={c.bg}>[{vimMode}]</span>
            </strong>
          </text>
          <text fg={c.dim} bg={c.bg}> | </text>
        </>
      )}
      <text bg={c.bg}>{model}</text>
      {runningAgents > 0 && (
        <>
          <text fg={c.dim} bg={c.bg}> | </text>
          <text fg="#A6E3A1" bg={c.bg}>{runningAgents} agent{runningAgents > 1 ? 's' : ''}</text>
        </>
      )}
      {activeTeams > 0 && (
        <>
          <text fg={c.dim} bg={c.bg}> | </text>
          <text fg="#CBA6F7" bg={c.bg}>{activeTeams} team{activeTeams > 1 ? 's' : ''}</text>
        </>
      )}
      {(connectedMcp > 0 || runningLsp > 0) && (
        <>
          <text fg={c.dim} bg={c.bg}> | </text>
          {runningLsp > 0 && <text fg="#89B4FA" bg={c.bg}>LSP:{runningLsp}</text>}
          {runningLsp > 0 && connectedMcp > 0 && <text fg={c.dim} bg={c.bg}>/</text>}
          {connectedMcp > 0 && <text fg="#CBA6F7" bg={c.bg}>MCP:{connectedMcp}</text>}
        </>
      )}
      {customError && (
        <>
          <text fg={c.dim} bg={c.bg}> | </text>
          <text fg="#F38BA8" bg={c.bg}>statusline: {customError}</text>
        </>
      )}
      {planWorkflow && planWorkflow.status !== 'completed' && (
        <>
          <text fg={c.dim} bg={c.bg}> | </text>
          <text fg="#FAB387" bg={c.bg}>plan:{planWorkflow.status}/{planWorkflow.approval_state}</text>
        </>
      )}
      <box flexGrow={1} />
      <text fg={c.dim} bg={c.bg}>Tokens: </text>
      <text bg={c.bg}>{formatTokens(usage.inputTokens + usage.outputTokens)}</text>
      <text fg={c.dim} bg={c.bg}> | Cost: </text>
      <text fg={c.success} bg={c.bg}>{formatCost(usage.costUsd)}</text>
    </box>
  )
}
