import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../../../theme.js'
import { Spinner } from '../../../Spinner.js'
import { applyGeneratedChunk } from '../../generateAgent.js'
import type { DraftAgent } from '../../types.js'

/**
 * Lite-native port of upstream's `wizard-steps/GenerateStep.tsx`.
 *
 * Upstream owns the full API streaming lifecycle here. In Lite the
 * backend performs the generation — this step accepts the description
 * input, lets the user press Enter to kick off generation, and then
 * renders whatever the caller feeds back via the `stream` prop. The
 * caller is responsible for wiring `stream` to an IPC subscription.
 */

type Props = {
  description: string
  /** Optional live-updating buffer of the partial generation result.
   *  When defined, the step renders a streaming preview. */
  stream?: string
  /** Called when the user triggers generation. Caller spins up the
   *  backend request and feeds updates back through `stream`. */
  onGenerate?: (description: string) => void
  /** Invoked once the stream settles. Receives the parsed patch. */
  onApply: (patch: Partial<DraftAgent>) => void
  onCancel: () => void
}

export function GenerateStep({
  description,
  stream,
  onGenerate,
  onApply,
  onCancel,
}: Props) {
  const [input, setInput] = useState(description)
  const [isGenerating, setIsGenerating] = useState(false)

  useEffect(() => {
    if (stream && stream.trim().length > 0) {
      const patch = applyGeneratedChunk(
        { agentType: '', description: '', systemPrompt: '' } as DraftAgent,
        stream,
      )
      if (patch.agentType || patch.systemPrompt) {
        setIsGenerating(false)
      }
    }
  }, [stream])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence

    if (name === 'escape') {
      onCancel()
      return
    }
    if (isGenerating) return

    if (name === 'backspace' || name === 'delete') {
      setInput(v => v.slice(0, -1))
      return
    }
    if (name === 'return' || name === 'enter') {
      if (input.trim().length === 0) return
      if (stream) {
        const patch = applyGeneratedChunk(
          { agentType: '', description: input, systemPrompt: '' } as DraftAgent,
          stream,
        )
        onApply({
          agentType: patch.agentType,
          description: patch.description || input,
          systemPrompt: patch.systemPrompt,
        })
        return
      }
      setIsGenerating(true)
      onGenerate?.(input)
      return
    }
    if (seq && seq.length === 1 && !event.ctrl && !event.meta) {
      setInput(v => v + seq)
    }
  })

  return (
    <box flexDirection="column" gap={1}>
      <strong><text fg={c.accent}>Describe the agent</text></strong>
      <text fg={c.dim}>
        In natural language, say what this agent should do. Enter to generate.
      </text>
      <box flexDirection="row" gap={1}>
        <text fg={c.accent}>{'\u276F'}</text>
        <text>{input || ' '}</text>
        <text fg={c.accent}>{'\u2588'}</text>
      </box>
      {isGenerating && <Spinner label="Generating…" />}
      {stream && (
        <box flexDirection="column" marginTop={1} paddingLeft={2}>
          <text fg={c.dim}>Preview:</text>
          <text>{stream.slice(0, 400)}</text>
        </box>
      )}
    </box>
  )
}
