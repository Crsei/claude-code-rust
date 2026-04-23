import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { AGENT_PATHS, type AgentSource } from './types.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/agentFileUtils.ts`.
 *
 * Upstream reads/writes YAML frontmatter markdown files directly from
 * the Ink process. The Lite frontend delegates that work to the Rust
 * backend (see `ipc/protocol.ts:FrontendMessage::agent_command`), so
 * this module only contains the path / label helpers the UI still uses
 * locally. IO helpers are stubs whose return values surface via the
 * backend's response events.
 */

export function getActualRelativeAgentFilePath(
  entry: AgentDefinitionEntry,
): string {
  if (entry.file_path) return entry.file_path
  const filename = `${entry.name}.md`
  if (entry.source?.kind === 'project') {
    return `${AGENT_PATHS.FOLDER_NAME}/${AGENT_PATHS.AGENTS_DIR}/${filename}`
  }
  if (entry.source?.kind === 'user') {
    return `~/${AGENT_PATHS.FOLDER_NAME}/${AGENT_PATHS.AGENTS_DIR}/${filename}`
  }
  return filename
}

/** The UI layer shouldn't delete files directly — always round-trip
 *  through the backend. Exposed as a promise-returning helper so call
 *  sites have a stable shape for future wiring. */
export async function deleteAgentFromFile(
  entry: AgentDefinitionEntry,
  sendCommand: (command: {
    kind: 'delete'
    agent_id: string
    source: AgentSource
  }) => void,
): Promise<void> {
  const source = (entry.source?.kind ?? 'user') as AgentSource
  sendCommand({
    kind: 'delete',
    agent_id: entry.name,
    source,
  })
}

/** Build a short human-readable label for the entry. */
export function describeAgent(entry: AgentDefinitionEntry): string {
  const kind = entry.source?.kind ?? 'unknown'
  return `${entry.name} (${kind})`
}

/** Given a source kind, return the canonical location label (for the
 *  `LocationStep` summary line). */
export function locationLabelForSource(source: AgentSource): string {
  switch (source) {
    case 'userSettings':
      return '~/.cc-rust/agents/'
    case 'projectSettings':
    case 'localSettings':
      return '.cc-rust/agents/'
    case 'policySettings':
      return '(policy)'
    case 'flagSettings':
      return '(flag)'
    case 'plugin':
      return '(plugin)'
    case 'built-in':
      return '(built-in)'
    case 'all':
      return ''
  }
}
