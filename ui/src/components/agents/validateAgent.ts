import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import type { AgentSource, AgentValidationResult, DraftAgent } from './types.js'
import { getAgentSourceDisplayName } from './utils.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/validateAgent.ts`.
 *
 * Upstream validated against an in-process `Tools` registry; the Lite
 * frontend can't walk the backend's tool registry synchronously, so
 * the tool validity check is simplified — callers can pass an
 * `availableTools` list (the IPC payload provides one) and we flag any
 * missing names as invalid.
 */

export function validateAgentType(agentType: string): string | null {
  if (!agentType) {
    return 'Agent type is required'
  }
  if (!/^[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9]$/.test(agentType)) {
    return 'Agent type must start and end with alphanumeric characters and contain only letters, numbers, and hyphens'
  }
  if (agentType.length < 3) {
    return 'Agent type must be at least 3 characters long'
  }
  if (agentType.length > 50) {
    return 'Agent type must be less than 50 characters'
  }
  return null
}

export function validateAgent(
  agent: DraftAgent,
  availableTools: string[],
  existingAgents: AgentDefinitionEntry[],
): AgentValidationResult {
  const errors: string[] = []
  const warnings: string[] = []

  if (!agent.agentType) {
    errors.push('Agent type is required')
  } else {
    const typeError = validateAgentType(agent.agentType)
    if (typeError) errors.push(typeError)

    const duplicate = existingAgents.find(
      a => a.name === agent.agentType && readSource(a) !== agent.source,
    )
    if (duplicate) {
      errors.push(
        `Agent type "${agent.agentType}" already exists in ${getAgentSourceDisplayName(
          readSource(duplicate) as AgentSource,
        )}`,
      )
    }
  }

  if (!agent.description) {
    errors.push('Description is required')
  } else if (agent.description.length < 10) {
    warnings.push('Description should be more descriptive (at least 10 characters)')
  } else if (agent.description.length > 5000) {
    warnings.push('Description is very long (over 5000 characters)')
  }

  if (agent.tools !== undefined && !Array.isArray(agent.tools)) {
    errors.push('Tools must be an array')
  } else if (agent.tools === undefined) {
    warnings.push('Agent has access to all tools')
  } else if (agent.tools.length === 0) {
    warnings.push('No tools selected — agent will have very limited capabilities')
  } else {
    const availableSet = new Set(availableTools)
    const invalid = agent.tools.filter(t => !availableSet.has(t))
    if (invalid.length > 0) {
      errors.push(`Invalid tools: ${invalid.join(', ')}`)
    }
  }

  if (!agent.systemPrompt) {
    errors.push('System prompt is required')
  } else if (agent.systemPrompt.length < 20) {
    errors.push('System prompt is too short (minimum 20 characters)')
  } else if (agent.systemPrompt.length > 10000) {
    warnings.push('System prompt is very long (over 10,000 characters)')
  }

  return {
    isValid: errors.length === 0,
    errors,
    warnings,
  }
}

function readSource(entry: AgentDefinitionEntry): string {
  return entry.source?.kind ?? 'unknown'
}
