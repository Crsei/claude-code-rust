import React from 'react'
import { c } from '../../../../theme.js'
import type { DraftAgent } from '../../types.js'
import { getAgentSourceDisplayName } from '../../utils.js'

/**
 * Lite-native port of upstream's `wizard-steps/ConfirmStep.tsx`.
 * Read-only review of the draft agent. The actual keyboard flow
 * (y/n confirm) lives in `ConfirmStepWrapper` so the read-only
 * preview can be reused outside the wizard.
 */

type Props = {
  draft: DraftAgent
}

function summarizeTools(tools: string[] | undefined): string {
  if (tools === undefined) return 'All tools (inherit)'
  if (tools.length === 0) return 'No tools'
  return tools.join(', ')
}

export function ConfirmStep({ draft }: Props) {
  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Review</text></strong>

      <Row label="Location" value={getAgentSourceDisplayName(draft.source)} />
      <Row label="Name" value={draft.agentType} />
      <Row label="Description" value={draft.description} />
      <Row label="Tools" value={summarizeTools(draft.tools)} />
      <Row label="Model" value={draft.model ?? '(inherit)'} />
      <Row label="Color" value={draft.color ?? 'automatic'} />
      <Row label="Memory" value={draft.memory ?? 'inherit'} />

      <box flexDirection="column" marginTop={1}>
        <strong><text>System prompt</text></strong>
        <box paddingLeft={2}>
          <text fg={c.dim}>
            {truncate(draft.systemPrompt, 400) || '(empty)'}
          </text>
        </box>
      </box>
    </box>
  )
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <box flexDirection="row" gap={1}>
      <strong><text>{label}:</text></strong>
      <text>{value || '—'}</text>
    </box>
  )
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + '\u2026'
}
