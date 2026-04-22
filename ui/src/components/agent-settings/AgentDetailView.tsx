import React from 'react'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import { isEditableSource, sourceColor, sourceLabel } from './constants.js'

/**
 * Read-only detail view for a single agent. Shown when the user highlights
 * an entry in the list and presses Enter. Edits happen via the form view.
 */

export interface AgentDetailViewProps {
  entry: AgentDefinitionEntry
}

export function AgentDetailView({ entry }: AgentDetailViewProps) {
  return (
    <box flexDirection="column">
      <text>
        <strong><span fg={c.textBright}>{entry.name}</span></strong>
        <span fg={c.dim}>{'  ·  '}</span>
        <span fg={sourceColor(entry.source)}>{sourceLabel(entry.source)}</span>
        {isEditableSource(entry.source) ? null : (
          <>
            <span fg={c.dim}>{'  ·  '}</span>
            <span fg={c.warning}>read-only</span>
          </>
        )}
      </text>

      {entry.file_path ? (
        <text><span fg={c.dim}>{entry.file_path}</span></text>
      ) : null}

      <box marginTop={1} flexDirection="column">
        <text>
          <strong>Description</strong>
          <span fg={c.dim}>{' (tells the orchestrator when to delegate)'}</span>
        </text>
        <box paddingLeft={2}>
          <text fg={c.text}>{entry.description || <span fg={c.dim}>(none)</span>}</text>
        </box>
      </box>

      <box marginTop={1} flexDirection="column">
        <text>
          <strong>Tools</strong>
          <span fg={c.dim}>{': '}</span>
          <span fg={c.text}>
            {entry.tools.length === 0 ? '(inherit all tools)' : entry.tools.join(', ')}
          </span>
        </text>
      </box>

      <box flexDirection="column">
        <text>
          <strong>Model</strong>
          <span fg={c.dim}>{': '}</span>
          <span fg={c.text}>{entry.model ?? '(inherit)'}</span>
        </text>
      </box>

      {entry.color ? (
        <box flexDirection="column">
          <text>
            <strong>Color</strong>
            <span fg={c.dim}>{': '}</span>
            <span fg={c.text}>{entry.color}</span>
          </text>
        </box>
      ) : null}

      {entry.system_prompt ? (
        <box marginTop={1} flexDirection="column">
          <text>
            <strong>System prompt</strong>
          </text>
          <box paddingLeft={2}>
            <text fg={c.text}>{previewPrompt(entry.system_prompt)}</text>
          </box>
        </box>
      ) : null}
    </box>
  )
}

/** Trim the system prompt for the detail pane — full text is in the file. */
function previewPrompt(prompt: string): string {
  const maxLines = 20
  const lines = prompt.trim().split(/\r?\n/)
  if (lines.length <= maxLines) return lines.join('\n')
  return lines.slice(0, maxLines).join('\n') + '\n…'
}
