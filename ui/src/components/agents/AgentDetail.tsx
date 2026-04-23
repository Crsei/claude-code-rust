import React from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { defaultSyntaxStyle } from '../opentui-syntax.js'
import { getActualRelativeAgentFilePath } from './agentFileUtils.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/AgentDetail.tsx`.
 *
 * Read-only agent detail pane shown when the user presses Enter on a
 * list row. The Lite port swaps upstream's `Markdown` for OpenTUI's
 * `<markdown>` primitive and drops the backend-owned fields (hooks,
 * memory scope) that aren't exposed over IPC yet.
 */

type Props = {
  agent: AgentDefinitionEntry
  onBack: () => void
}

export function AgentDetail({ agent, onBack }: Props) {
  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'escape' || name === 'return' || name === 'enter') {
      onBack()
    }
  })

  const filePath = getActualRelativeAgentFilePath(agent)

  return (
    <box flexDirection="column" gap={1}>
      <text fg={c.dim}>{filePath}</text>

      <box flexDirection="column">
        <text>
          <strong>Description</strong>{' '}
          <span fg={c.dim}>(tells Claude when to use this agent):</span>
        </text>
        <box marginLeft={2}>
          <text>{agent.description || ''}</text>
        </box>
      </box>

      <box flexDirection="row">
        <text><strong>Tools</strong>: </text>
        {agent.tools.length === 0 ? (
          <text>All tools</text>
        ) : (
          <text>{agent.tools.join(', ')}</text>
        )}
      </box>

      <text>
        <strong>Model</strong>: {agent.model ?? '(inherit)'}
      </text>

      {agent.permission_mode && (
        <text>
          <strong>Permission mode</strong>: {agent.permission_mode}
        </text>
      )}

      {agent.memory && (
        <text>
          <strong>Memory</strong>: {agent.memory}
        </text>
      )}

      {agent.skills && agent.skills.length > 0 && (
        <text>
          <strong>Skills</strong>: {agent.skills.length > 10 ? `${agent.skills.length} skills` : agent.skills.join(', ')}
        </text>
      )}

      {agent.color && (
        <text>
          <strong>Color</strong>: {agent.color}
        </text>
      )}

      {agent.system_prompt && (
        <>
          <text><strong>System prompt</strong>:</text>
          <box marginLeft={2} marginRight={2}>
            <markdown content={agent.system_prompt} syntaxStyle={defaultSyntaxStyle} />
          </box>
        </>
      )}

      <box marginTop={1}>
        <text fg={c.dim}>Press Enter or Esc to go back</text>
      </box>
    </box>
  )
}
