import React, { useCallback, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { AgentDetail } from './AgentDetail.js'
import { AgentEditor } from './AgentEditor.js'
import { AgentNavigationFooter } from './AgentNavigationFooter.js'
import { AgentsList } from './AgentsList.js'
import { CreateAgentWizard } from './new-agent-creation/CreateAgentWizard.js'
import type { DraftAgent, ModeState } from './types.js'
import type { ToolSpec } from './ToolSelector.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/AgentsMenu.tsx`.
 *
 * Top-level entry point for the agent management UI. Owns the mode
 * state machine (list → view → edit → create → delete-confirm) and
 * wires each sub-panel. IPC plumbing (create / save / delete) is
 * pushed to props so the dialog is transport-agnostic — upstream has
 * the same structure but with `useSetAppState` mutations instead of
 * callbacks.
 */

type Props = {
  agents: AgentDefinitionEntry[]
  availableTools: ToolSpec[]
  onCreate: (draft: DraftAgent) => void
  onUpdate: (entry: AgentDefinitionEntry, draft: DraftAgent) => void
  onDelete: (entry: AgentDefinitionEntry) => void
  onExit: (message?: string) => void
}

export function AgentsMenu({
  agents,
  availableTools,
  onCreate,
  onUpdate,
  onDelete,
  onExit,
}: Props) {
  const [mode, setMode] = useState<ModeState>({ mode: 'list-agents', source: 'all' })
  const [changes, setChanges] = useState<string[]>([])

  const back = useCallback(() => setMode({ mode: 'list-agents', source: 'all' }), [])

  if (mode.mode === 'list-agents') {
    return (
      <box flexDirection="column" gap={1}>
        <strong><text fg={c.accent}>Agents</text></strong>
        {changes.length > 0 && (
          <box flexDirection="column">
            {changes.map((msg, i) => (
              <text key={i} fg={c.success}>• {msg}</text>
            ))}
          </box>
        )}
        <AgentsList
          agents={agents}
          filter={mode.source}
          onSelect={entry =>
            setMode({ mode: 'view-agent', agent: entry, previousMode: mode })
          }
          onCancel={() => onExit()}
        />
        <box flexDirection="row" gap={2} marginTop={1}>
          <text fg={c.dim}>Press <strong>n</strong> to create a new agent</text>
        </box>
        <NewAgentListener onNew={() => setMode({ mode: 'create-agent' })} />
        <AgentNavigationFooter />
      </box>
    )
  }

  if (mode.mode === 'view-agent') {
    return (
      <box flexDirection="column" gap={1}>
        <strong><text fg={c.accent}>{mode.agent.name}</text></strong>
        <AgentDetail agent={mode.agent} onBack={back} />
        <box flexDirection="row" gap={2} marginTop={1}>
          <text fg={c.dim}>e: edit · d: delete · Esc: back</text>
        </box>
        <AgentActionListener
          onEdit={() =>
            setMode({ mode: 'edit-agent', agent: mode.agent, previousMode: mode })
          }
          onDelete={() =>
            setMode({
              mode: 'delete-confirm',
              agent: mode.agent,
              previousMode: mode,
            })
          }
        />
      </box>
    )
  }

  if (mode.mode === 'edit-agent') {
    return (
      <AgentEditor
        agent={mode.agent}
        availableTools={availableTools}
        existingAgents={agents}
        onSave={draft => {
          onUpdate(mode.agent, draft)
          setChanges(prev => [...prev, `Updated ${draft.agentType}`])
          back()
        }}
        onCancel={back}
      />
    )
  }

  if (mode.mode === 'create-agent') {
    return (
      <CreateAgentWizard
        availableTools={availableTools}
        existingAgents={agents}
        onDone={draft => {
          onCreate(draft)
          setChanges(prev => [...prev, `Created ${draft.agentType}`])
          back()
        }}
        onCancel={back}
      />
    )
  }

  if (mode.mode === 'delete-confirm') {
    return (
      <box flexDirection="column" gap={1}>
        <strong><text fg={c.warning}>Delete agent?</text></strong>
        <text>
          Are you sure you want to delete <strong>{mode.agent.name}</strong>?
        </text>
        <text fg={c.dim}>This cannot be undone.</text>
        <DeleteConfirmListener
          onYes={() => {
            onDelete(mode.agent)
            setChanges(prev => [...prev, `Deleted ${mode.agent.name}`])
            back()
          }}
          onNo={back}
        />
        <text fg={c.dim}>y: confirm · n / Esc: cancel</text>
      </box>
    )
  }

  return null
}

// Tiny helper components for isolated keyboard listeners. Using nested
// `useKeyboard` hooks from each call site keeps the listener active only
// while that view is mounted, avoiding cross-mode leakage.

function NewAgentListener({ onNew }: { onNew: () => void }) {
  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const seq = event.sequence?.toLowerCase()
    if (seq === 'n') onNew()
  })
  return null
}

function AgentActionListener({
  onEdit,
  onDelete,
}: {
  onEdit: () => void
  onDelete: () => void
}) {
  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const seq = event.sequence?.toLowerCase()
    if (seq === 'e') onEdit()
    else if (seq === 'd') onDelete()
  })
  return null
}

function DeleteConfirmListener({
  onYes,
  onNo,
}: {
  onYes: () => void
  onNo: () => void
}) {
  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const seq = event.sequence?.toLowerCase()
    if (seq === 'y') onYes()
    else if (seq === 'n' || event.name === 'escape') onNo()
  })
  return null
}
