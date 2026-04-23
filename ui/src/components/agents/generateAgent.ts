import type { DraftAgent } from './types.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/generateAgent.ts`.
 *
 * Upstream calls the Anthropic API to write a system prompt from the
 * user's description. In Lite we delegate to the backend; callers send
 * an `agent_command` with `kind: 'generate'` and the backend streams a
 * response. This module exposes the helpers that assemble the draft
 * payload + parse a streamed chunk back into the editor buffer.
 */

const GENERATE_INSTRUCTIONS = `
You are helping a developer create a Claude Code sub-agent.
Given the user's natural-language description, produce:
- a short agent name in kebab-case,
- a one-sentence "when to use" blurb,
- a concise system prompt (20-200 words).
Return ONLY valid JSON of shape { name, description, systemPrompt }.
`.trim()

export function buildGeneratePayload(description: string): {
  instructions: string
  description: string
} {
  return {
    instructions: GENERATE_INSTRUCTIONS,
    description: description.trim(),
  }
}

/**
 * Incrementally merge a streamed JSON chunk into the in-progress draft.
 * Upstream uses a proper streaming parser; this helper takes the last
 * snapshot of buffered text and attempts a full `JSON.parse`, ignoring
 * parse errors while the JSON is still incomplete.
 */
export function applyGeneratedChunk(
  draft: DraftAgent,
  buffer: string,
): DraftAgent {
  const trimmed = buffer.trim()
  if (!trimmed.startsWith('{')) return draft
  try {
    const parsed = JSON.parse(trimmed) as Partial<{
      name: string
      description: string
      systemPrompt: string
    }>
    return {
      ...draft,
      agentType: parsed.name ?? draft.agentType,
      description: parsed.description ?? draft.description,
      systemPrompt: parsed.systemPrompt ?? draft.systemPrompt,
    }
  } catch {
    return draft
  }
}
