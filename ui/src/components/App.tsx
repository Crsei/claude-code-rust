import React, { useEffect } from 'react'
import { AlternateScreen, Box } from 'ink-terminal'
import { useBackend } from '../ipc/context.js'
import { useAppState, useAppDispatch } from '../store/app-store.js'
import { Header } from './Header.js'
import { MessageList } from './MessageList.js'
import { InputPrompt } from './InputPrompt.js'
import { StatusBar } from './StatusBar.js'
import { WelcomeScreen } from './WelcomeScreen.js'
import { PermissionDialog } from './PermissionDialog.js'
import { Suggestions } from './Suggestions.js'
import { Spinner } from './Spinner.js'
import type { BackendMessage } from '../ipc/protocol.js'

function extractTextFromContent(content: any): string {
  if (Array.isArray(content)) {
    return content
      .filter((block: any) => block.type === 'text')
      .map((block: any) => block.text || '')
      .join('\n')
  }
  if (typeof content === 'string') return content
  return JSON.stringify(content)
}

export function App() {
  const backend = useBackend()
  const state = useAppState()
  const dispatch = useAppDispatch()

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
        case 'stream_end':
          dispatch({ type: 'STREAM_END' })
          break
        case 'assistant_message':
          dispatch({
            type: 'ASSISTANT_MESSAGE',
            id: msg.id,
            content: extractTextFromContent(msg.content),
            costUsd: msg.cost_usd,
          })
          break
        case 'tool_use':
          dispatch({ type: 'TOOL_USE', id: msg.id, name: msg.name, input: msg.input })
          break
        case 'tool_result':
          dispatch({
            type: 'TOOL_RESULT',
            toolUseId: msg.tool_use_id,
            output: msg.output,
            isError: msg.is_error,
          })
          break
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
        case 'usage_update':
          dispatch({
            type: 'USAGE_UPDATE',
            usage: { inputTokens: msg.input_tokens, outputTokens: msg.output_tokens, costUsd: msg.cost_usd },
          })
          break
        case 'suggestions':
          dispatch({ type: 'SUGGESTIONS', items: msg.items })
          break
        case 'error':
          dispatch({ type: 'ERROR', message: msg.message })
          break
      }
    }

    backend.on('message', handler)
    return () => { backend.removeListener('message', handler) }
  }, [backend, dispatch])

  const isWelcome = state.messages.length === 0 && !state.isStreaming && !state.isWaiting
  const isBusy = state.isWaiting || state.isStreaming

  return (
    <AlternateScreen mouseTracking>
      <Box flexDirection="column" height="100%">
        <Header model={state.model} sessionId={state.sessionId} />
        {isWelcome ? (
          <Box flexGrow={1} flexDirection="column" alignItems="center" justifyContent="center">
            <WelcomeScreen />
            <Box marginTop={1}>
              <InputPrompt />
            </Box>
          </Box>
        ) : (
          <>
            <Box flexGrow={1}>
              <MessageList />
            </Box>
            {state.suggestions.length > 0 && !isBusy && <Suggestions />}
            {isBusy && <Spinner label={state.isStreaming && state.streamingText ? 'Reasoning...' : 'Thinking...'} />}
            {!isBusy && <InputPrompt />}
          </>
        )}
        <StatusBar usage={state.usage} model={state.model} vimMode={state.vimEnabled ? state.vimMode : undefined} />
        {state.permissionRequest && <PermissionDialog request={state.permissionRequest} />}
      </Box>
    </AlternateScreen>
  )
}
