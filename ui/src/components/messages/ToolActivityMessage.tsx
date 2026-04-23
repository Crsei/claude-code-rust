import React from 'react'
import type { ViewMode } from '../../keybindings.js'
import { useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'
import type { ToolActivityRenderItem } from '../../store/message-model.js'
import type { ToolStatus } from '../../view-model/types.js'
import { defaultSyntaxStyle } from '../opentui-syntax.js'
import { ShellProgressMessage } from '../shell/index.js'
import { FileEditToolPreview, isFileEditToolName } from './FileEditToolPreview.js'

/**
 * OpenTUI port of the upstream `AssistantToolUseMessage` +
 * `UserToolResultMessage` pair:
 * - `ui/examples/upstream-patterns/src/components/messages/AssistantToolUseMessage.tsx`
 * - `ui/examples/upstream-patterns/src/components/messages/UserToolResultMessage/`
 *
 * Upstream shows a status glyph (● for queued, animated loader while running,
 * ✓ on success, ✗ on error) followed by a bold tool name and a parenthesised
 * input summary. We reproduce that shape here — OpenTUI doesn't animate, so
 * `running` gets a filled circle and `pending` a hollow one. The status-style
 * table keys on `ToolStatus` from the view-model layer (Issue 01 adapter) so
 * it stays aligned with `classifyToolStatus`.
 */

type Props = {
  item: ToolActivityRenderItem
  viewMode: ViewMode
}

interface StatusStyle {
  label: string
  color: string
  glyph: string
}

const STATUS_STYLES: Record<ToolStatus, StatusStyle> = {
  pending:   { label: 'PENDING',   color: c.dim,     glyph: '\u25CC' }, // ◌
  running:   { label: 'RUN',       color: c.info,    glyph: '\u25CF' }, // ●
  success:   { label: 'OK',        color: c.success, glyph: '\u2713' }, // ✓
  error:     { label: 'ERROR',     color: c.error,   glyph: '\u2717' }, // ✗
  cancelled: { label: 'CANCELLED', color: c.warning, glyph: '\u25A0' }, // ■
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
    >
      <text fg={c.warning}>
        <strong>Question</strong>
      </text>
      <markdown content={question} syntaxStyle={defaultSyntaxStyle} />
    </box>
  )
}

export function ToolActivityMessage({ item, viewMode }: Props) {
  const status = STATUS_STYLES[item.status]
  const question = extractAskUserQuestion(item)
  const showEditPreview = isFileEditToolName(item.name)
  const { shellProgress } = useAppState()
  const bashProgress =
    item.name === 'Bash' ? shellProgress[item.toolUseId] : undefined
  const showShellProgress = Boolean(bashProgress)

  if (viewMode === 'prompt') {
    return (
      <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
        <box flexDirection="row" gap={1} width="100%">
          <box minWidth={2} flexShrink={0}>
            <text fg={status.color}>{status.glyph}</text>
          </box>
          <text fg={c.warning}>
            <strong>{item.name}</strong>
          </text>
          {!question && !showEditPreview && (
            <text fg={item.isError ? c.error : c.dim} selectable>
              ({item.inputSummary || '(no input)'})
            </text>
          )}
        </box>
        {showShellProgress && bashProgress && (
          <box paddingLeft={3} width="100%">
            <ShellProgressMessage
              output={bashProgress.output}
              fullOutput={bashProgress.fullOutput}
              elapsedTimeSeconds={bashProgress.elapsedSeconds}
              totalLines={bashProgress.totalLines}
              totalBytes={bashProgress.totalBytes}
              timeoutMs={bashProgress.timeoutMs}
              verbose={false}
            />
          </box>
        )}
        {item.outputSummary && !question && !showEditPreview && !showShellProgress && (
          <box paddingLeft={3} width="100%" flexDirection="row">
            <text fg={c.dim}>{'\u23BF '}</text>
            <text selectable fg={item.isError ? c.error : c.dim}>
              {item.outputSummary}
            </text>
          </box>
        )}
        {showEditPreview && (
          <box paddingLeft={3} width="100%">
            <FileEditToolPreview toolName={item.name} input={item.input} />
          </box>
        )}
        {question && (
          <box paddingLeft={3} paddingTop={1} width="100%">
            <AskUserQuestionCallout question={question} />
          </box>
        )}
        {question && item.outputSummary && (
          <box paddingLeft={3} paddingTop={1} flexDirection="column" width="100%">
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
          <text fg={status.color}>{status.glyph}</text>
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
          {showShellProgress && bashProgress ? (
            <>
              <text fg={c.dim}>Progress</text>
              <ShellProgressMessage
                output={bashProgress.output}
                fullOutput={bashProgress.fullOutput}
                elapsedTimeSeconds={bashProgress.elapsedSeconds}
                totalLines={bashProgress.totalLines}
                totalBytes={bashProgress.totalBytes}
                timeoutMs={bashProgress.timeoutMs}
                verbose={true}
              />
            </>
          ) : (
            <>
              <text fg={c.dim}>Result</text>
              <text selectable fg={item.isError ? c.error : c.text}>
                {item.outputSummary || '(waiting for result)'}
              </text>
            </>
          )}
        </box>
      </box>
    </box>
  )
}
