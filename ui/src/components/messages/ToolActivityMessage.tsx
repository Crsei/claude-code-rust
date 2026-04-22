import React from 'react'
import type { ViewMode } from '../../keybindings.js'
import { c } from '../../theme.js'
import type { ToolActivityRenderItem } from '../../store/message-model.js'
import type { ToolStatus } from '../../view-model/types.js'
import { FileEditToolPreview, isFileEditToolName } from './FileEditToolPreview.js'

/**
 * Lite-native port of the sample tree's `AssistantToolUseMessage` +
 * `UserToolResultMessage` pair, collapsed into one leaf that reads the
 * current render item's merged `status` / `outputSummary`. See:
 * - `ui/examples/upstream-patterns/src/components/messages/AssistantToolUseMessage.tsx`
 * - `ui/examples/upstream-patterns/src/components/messages/UserToolResultMessage/`
 *
 * The status-style table uses `ToolStatus` from the view-model layer
 * (Issue 01 adapter), so the closed set here stays in sync with the
 * adapter's `classifyToolStatus` output.
 */

type Props = {
  item: ToolActivityRenderItem
  viewMode: ViewMode
}

const STATUS_STYLES: Record<ToolStatus, { label: string; color: string }> = {
  pending: { label: 'PENDING', color: c.dim },
  running: { label: 'RUN', color: c.info },
  success: { label: 'OK', color: c.success },
  error: { label: 'ERROR', color: c.error },
  cancelled: { label: 'CANCELLED', color: c.warning },
}

function extractAskUserQuestion(item: ToolActivityRenderItem): string | undefined {
  if (item.name !== 'AskUserQuestion') {
    return undefined
  }
  const question = item.input?.question
  return typeof question === 'string' && question.trim() ? question.trim() : undefined
}

function AskUserQuestionCallout({ question }: { question: string }) {
  return (
    <box
      border={['left']}
      borderColor={c.toolQuestionBorder}
      backgroundColor={c.toolQuestionBg}
      paddingLeft={1}
      paddingRight={1}
      flexDirection="column"
      width="100%"
      selectable
    >
      <text fg={c.warning}>
        <strong>Question</strong>
      </text>
      <markdown content={question} />
    </box>
  )
}

export function ToolActivityMessage({ item, viewMode }: Props) {
  const status = STATUS_STYLES[item.status]
  const question = extractAskUserQuestion(item)
  const showEditPreview = isFileEditToolName(item.name)
  const detail = item.outputSummary
    ? `${item.inputSummary} -> ${item.outputSummary}`
    : item.inputSummary || '(no input summary)'

  if (viewMode === 'prompt') {
    return (
      <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
        <box gap={1} width="100%">
          <text fg={status.color}>
            <strong>[{status.label}]</strong>
          </text>
          <text fg={c.warning}>
            <strong>{item.name}</strong>
          </text>
          {!question && !showEditPreview && (
            <text fg={item.isError ? c.error : c.dim} selectable>
              {detail}
            </text>
          )}
        </box>
        {showEditPreview && (
          <box paddingLeft={2} width="100%">
            <FileEditToolPreview toolName={item.name} input={item.input} />
          </box>
        )}
        {question && (
          <box paddingLeft={2} paddingTop={1} width="100%">
            <AskUserQuestionCallout question={question} />
          </box>
        )}
        {question && item.outputSummary && (
          <box paddingLeft={2} paddingTop={1} flexDirection="column" width="100%">
            <text fg={c.dim}>Answer</text>
            <text selectable fg={item.isError ? c.error : c.text}>
              {item.outputSummary}
            </text>
          </box>
        )}
      </box>
    )
  }

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box border={['left']} borderColor={status.color} paddingLeft={1} flexDirection="column">
        <box gap={1}>
          <text fg={status.color}>
            <strong>[{status.label}]</strong>
          </text>
          <text fg={c.warning}>
            <strong>{item.name}</strong>
          </text>
          <text fg={c.dim}>({item.toolUseId.slice(0, 8)})</text>
        </box>
        <box paddingLeft={2} flexDirection="column" width="100%">
          <text fg={c.dim}>{question ? 'Question' : 'Input'}</text>
          {question
            ? <AskUserQuestionCallout question={question} />
            : <text selectable>{item.inputDetail || item.inputSummary || '(no input summary)'}</text>}
          {showEditPreview && (
            <FileEditToolPreview toolName={item.name} input={item.input} />
          )}
          <text fg={c.dim}>Result</text>
          <text selectable fg={item.isError ? c.error : c.text}>
            {item.outputSummary || '(waiting for result)'}
          </text>
        </box>
      </box>
    </box>
  )
}
