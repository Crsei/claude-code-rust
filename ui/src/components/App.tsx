import React, { useEffect, useRef, useState } from 'react'
import { useTerminalDimensions } from '@opentui/react'
import { useBackend } from '../ipc/context.js'
import type { BackendMessage, FrontendContentBlock } from '../ipc/protocol.js'
import { shortcutLabel, transcriptTitle } from '../keybindings.js'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import { conversationToRawMessage } from '../store/message-model.js'
import { Header } from './Header.js'
import { InputPrompt } from './InputPrompt.js'
import { MessageList } from './MessageList.js'
import { PermissionDialog } from './PermissionDialog.js'
import { Suggestions } from './Suggestions.js'
import { WelcomeScreen } from './WelcomeScreen.js'

type ActivePane = 'messages' | 'input'

function extractFromContent(
  content: FrontendContentBlock[] | string | null,
): { text: string; thinking: string; blocks?: FrontendContentBlock[] } {
  if (Array.isArray(content)) {
    const textParts: string[] = []
    const thinkingParts: string[] = []

    for (const block of content) {
      if (block.type === 'text') {
        textParts.push(block.text)
      } else if (block.type === 'thinking') {
        thinkingParts.push(block.thinking)
      }
    }

    return {
      text: textParts.join('\n'),
      thinking: thinkingParts.join('\n'),
      blocks: content,
    }
  }

  if (typeof content === 'string') {
    return { text: content, thinking: '' }
  }

  return { text: '', thinking: '' }
}

function composerHint(isTranscript: boolean): string {
  if (isTranscript) {
    return transcriptTitle()
  }

  return [
    `${shortcutLabel('app.toggleTranscript')} transcript`,
    `${shortcutLabel('app.toggleVim')} vim`,
    `${shortcutLabel('app.quit')} quit`,
  ].join(' | ')
}

export function App() {
  const backend = useBackend()
  const state = useAppState()
  const dispatch = useAppDispatch()
  const [activePane, setActivePane] = useState<ActivePane>('input')
  const [inputStatus, setInputStatus] = useState('')
  const lastPromptPaneRef = useRef<ActivePane>('input')
  const { width: termWidth } = useTerminalDimensions()
  const welcomePromptWidth = Math.min(84, Math.max(40, termWidth - 8))

  useEffect(() => {
    const handler = (msg: BackendMessage) => {
      switch (msg.type) {
        case 'ready':
          dispatch({ type: 'READY', model: msg.model, sessionId: msg.session_id, cwd: msg.cwd })
          break
        case 'stream_start':
          dispatch({ type: 'STREAM_START', messageId: msg.message_id })
          break
        case 'stream_delta':
          dispatch({ type: 'STREAM_DELTA', text: msg.text })
          break
        case 'thinking_delta':
          dispatch({ type: 'THINKING_DELTA', thinking: msg.thinking })
          break
        case 'stream_end':
          dispatch({ type: 'STREAM_END' })
          break
        case 'assistant_message': {
          const { text, thinking, blocks } = extractFromContent(msg.content)
          dispatch({
            type: 'ASSISTANT_MESSAGE',
            id: msg.id,
            content: text,
            contentBlocks: blocks,
            costUsd: msg.cost_usd,
            thinking: thinking || undefined,
          })
          break
        }
        case 'tool_use':
          dispatch({ type: 'TOOL_USE', id: msg.id, name: msg.name, input: msg.input })
          break
        case 'tool_result': {
          dispatch({
            type: 'TOOL_RESULT',
            toolUseId: msg.tool_use_id,
            output: msg.output,
            isError: msg.is_error,
          })
          const bgMatch = msg.output.match(/Agent '(.+?)' launched in background \(id: (.+?)\)/)
          if (bgMatch) {
            dispatch({
              type: 'BG_AGENT_STARTED',
              agentId: bgMatch[2]!,
              description: bgMatch[1]!,
            })
          }
          break
        }
        case 'permission_request':
          dispatch({
            type: 'PERMISSION_REQUEST',
            request: {
              toolUseId: msg.tool_use_id,
              tool: msg.tool,
              command: msg.command,
              options: msg.options,
            },
          })
          break
        case 'system_info':
          dispatch({ type: 'SYSTEM_INFO', text: msg.text, level: msg.level })
          break
        case 'conversation_replaced':
          dispatch({
            type: 'REPLACE_MESSAGES',
            messages: msg.messages.map(conversationToRawMessage),
          })
          break
        case 'usage_update':
          dispatch({
            type: 'USAGE_UPDATE',
            usage: {
              inputTokens: msg.input_tokens,
              outputTokens: msg.output_tokens,
              costUsd: msg.cost_usd,
            },
          })
          break
        case 'suggestions':
          dispatch({ type: 'SUGGESTIONS', items: msg.items })
          break
        case 'background_agent_complete':
          dispatch({
            type: 'BG_AGENT_COMPLETE',
            agentId: msg.agent_id,
            description: msg.description,
            resultPreview: msg.result_preview,
            hadError: msg.had_error,
            durationMs: msg.duration_ms,
          })
          break
        case 'error':
          dispatch({ type: 'ERROR', message: msg.message })
          break
      }
    }

    backend.on('message', handler)
    return () => {
      backend.removeListener('message', handler)
    }
  }, [backend, dispatch])

  const isWelcome = state.messages.length === 0 && !state.isStreaming && !state.isWaiting
  const isBusy = state.isWaiting || state.isStreaming
  const isTranscript = state.viewMode === 'transcript'
  const queuedCount = state.queuedSubmissions.length

  useEffect(() => {
    if (isWelcome) {
      setActivePane('input')
      lastPromptPaneRef.current = 'input'
    }
  }, [isWelcome])

  useEffect(() => {
    if (state.viewMode === 'prompt') {
      lastPromptPaneRef.current = activePane
    }
  }, [activePane, state.viewMode])

  useEffect(() => {
    if (state.viewMode === 'transcript') {
      setActivePane('messages')
    } else {
      setActivePane(lastPromptPaneRef.current)
    }
  }, [state.viewMode])

  useEffect(() => {
    if (isBusy || state.permissionRequest || queuedCount === 0) {
      return
    }

    const next = state.queuedSubmissions[0]
    if (!next) {
      return
    }

    dispatch({ type: 'DEQUEUE_SUBMISSION' })
    dispatch({ type: 'ADD_USER_MESSAGE', id: next.id, text: next.text })
    backend.send({ type: 'submit_prompt', text: next.text, id: next.id })
  }, [backend, dispatch, isBusy, queuedCount, state.permissionRequest, state.queuedSubmissions])

  const queueSuffix = queuedCount > 0 ? ` | queued ${queuedCount}` : ''
  const inputTitle = isTranscript
    ? composerHint(true)
    : `${inputStatus || composerHint(false)}${queueSuffix}`

  return (
    <box flexDirection="column" height="100%">
      {!isWelcome && (
        <Header
          cwd={state.cwd}
          model={state.model}
          usage={state.usage}
          vimMode={state.vimEnabled ? state.vimMode : undefined}
          viewMode={state.viewMode}
        />
      )}
      {isWelcome ? (
        <box flexGrow={1} flexDirection="column" alignItems="center" justifyContent="center">
          <WelcomeScreen />
          <box
            marginTop={1}
            width={welcomePromptWidth}
            border
            borderStyle="rounded"
            borderColor="#45475A"
            paddingX={1}
            title={composerHint(false)}
            titleAlignment="right"
          >
            <InputPrompt isActive onActivate={() => setActivePane('input')} viewMode="prompt" />
          </box>
          <box marginTop={1}>
            <text fg="#45475A">
              <em>{shortcutLabel('app.quit')} to quit</em>
            </text>
          </box>
        </box>
      ) : (
        <box flexGrow={1} flexDirection="column">
          <box flexGrow={1}>
            <MessageList
              isActive={isTranscript || activePane === 'messages'}
              onActivate={() => setActivePane('messages')}
              viewMode={state.viewMode}
            >
              {state.suggestions.length > 0 && !isBusy && state.viewMode === 'prompt' && <Suggestions />}
            </MessageList>
          </box>
          <box
            width="100%"
            border
            borderStyle="rounded"
            borderColor="#45475A"
            paddingX={1}
            title={inputTitle}
            titleAlignment="right"
          >
            <InputPrompt
              isActive={!isTranscript && activePane === 'input'}
              isReadOnly={isTranscript}
              viewMode={state.viewMode}
              onActivate={() => setActivePane('input')}
              onStatusChange={setInputStatus}
            />
          </box>
        </box>
      )}
      {state.permissionRequest && <PermissionDialog request={state.permissionRequest} />}
    </box>
  )
}
