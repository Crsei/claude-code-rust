/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/types.ts`.
 *
 * Upstream types hang off internal Ink/Tool-system modules. The Lite
 * port re-expresses them against the IPC `AgentDefinitionEntry` shape
 * the backend already broadcasts, so the frontend doesn't need to
 * re-implement the `AgentDefinition` class.
 */

import type { AgentDefinitionEntry } from '../../ipc/protocol.js'

export const AGENT_PATHS = {
  FOLDER_NAME: '.cc-rust',
  AGENTS_DIR: 'agents',
} as const

/**
 * A scope identifier. Matches upstream's settings-source strings, plus
 * synthetic `all` / `built-in` / `plugin` aggregates used by the list
 * view.
 */
export type AgentSource =
  | 'userSettings'
  | 'projectSettings'
  | 'policySettings'
  | 'localSettings'
  | 'flagSettings'
  | 'plugin'
  | 'built-in'
  | 'all'

type WithPreviousMode = { previousMode: ModeState }
type WithAgent = { agent: AgentDefinitionEntry }

export type ModeState =
  | { mode: 'main-menu' }
  | { mode: 'list-agents'; source: AgentSource }
  | ({ mode: 'agent-menu' } & WithAgent & WithPreviousMode)
  | ({ mode: 'view-agent' } & WithAgent & WithPreviousMode)
  | { mode: 'create-agent' }
  | ({ mode: 'edit-agent' } & WithAgent & WithPreviousMode)
  | ({ mode: 'delete-confirm' } & WithAgent & WithPreviousMode)

export type AgentValidationResult = {
  isValid: boolean
  warnings: string[]
  errors: string[]
}

/**
 * The shape that wizard steps assemble incrementally before sending a
 * `create` IPC message. Matches upstream's `CustomAgentDefinition`
 * minus the disk-writer fields.
 */
export type DraftAgent = {
  agentType: string
  description: string
  systemPrompt: string
  tools?: string[]
  model?: string
  color?: string
  memory?: string
  permissionMode?: string
  /** Source the agent should be written to (user / project / etc). */
  source: AgentSource
}
