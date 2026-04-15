import React, { useState, useEffect, useCallback } from 'react'
import { Box, Text, useInput } from 'ink-terminal/react'
import { useBackend } from '../ipc/context.js'
import { useAppState, useAppDispatch } from '../store/app-store.js'
import { conversationToRawMessage } from '../store/message-model.js'
import type { BackendMessage } from '../ipc/protocol.js'

export default function App() {
  const backend = useBackend()
  const state = useAppState()
  const dispatch = useAppDispatch()
  const [inputText, setInputText] = useState('')

  // ---------------------------------------------------------------------------
  // Wire up backend messages → dispatch
  // ---------------------------------------------------------------------------

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
          const text = msg.content?.map(b => b.type === 'text' ? b.text : '').join('') ?? ''
          dispatch({ type: 'ASSISTANT_MESSAGE', id: msg.id, content: text, contentBlocks: msg.content ?? undefined, costUsd: msg.cost_usd })
          break
        }
        case 'tool_use':
          dispatch({ type: 'TOOL_USE', id: msg.id, name: msg.name, input: msg.input })
          break
        case 'tool_result':
          dispatch({ type: 'TOOL_RESULT', toolUseId: msg.tool_use_id, output: msg.output, isError: msg.is_error })
          break
        case 'permission_request':
          dispatch({ type: 'PERMISSION_REQUEST', request: { toolUseId: msg.tool_use_id, tool: msg.tool, command: msg.command, options: msg.options } })
          break
        case 'system_info':
          dispatch({ type: 'SYSTEM_INFO', text: msg.text, level: msg.level })
          break
        case 'conversation_replaced':
          dispatch({ type: 'REPLACE_MESSAGES', messages: msg.messages.map(conversationToRawMessage) })
          break
        case 'usage_update':
          dispatch({ type: 'USAGE_UPDATE', usage: { inputTokens: msg.input_tokens, outputTokens: msg.output_tokens, costUsd: msg.cost_usd } })
          break
        case 'suggestions':
          dispatch({ type: 'SUGGESTIONS', items: msg.items })
          break
        case 'error':
          dispatch({ type: 'ERROR', message: msg.message })
          break
        case 'background_agent_complete':
          dispatch({ type: 'BG_AGENT_COMPLETE', agentId: msg.agent_id, description: msg.description, resultPreview: msg.result_preview, hadError: msg.had_error, durationMs: msg.duration_ms })
          break
      }
    }

    backend.on('message', handler)
    return () => { backend.removeListener('message', handler) }
  }, [backend, dispatch])

  // ---------------------------------------------------------------------------
  // Input handling
  // ---------------------------------------------------------------------------

  const submit = useCallback(() => {
    const text = inputText.trim()
    if (!text) return

    if (text.startsWith('/')) {
      backend.send({ type: 'slash_command', raw: text })
      dispatch({ type: 'ADD_COMMAND_MESSAGE', id: crypto.randomUUID(), text })
    } else {
      const id = crypto.randomUUID()
      backend.send({ type: 'submit_prompt', text, id })
      dispatch({ type: 'ADD_USER_MESSAGE', id, text })
      dispatch({ type: 'PUSH_HISTORY', text })
    }
    setInputText('')
  }, [inputText, backend, dispatch])

  useInput((input: string, key: any) => {
    // Permission handling: y/n
    if (state.permissionRequest) {
      if (input === 'y' || input === 'Y') {
        backend.send({ type: 'permission_response', tool_use_id: state.permissionRequest.toolUseId, decision: 'allow' })
        dispatch({ type: 'PERMISSION_DISMISS' })
      } else if (input === 'n' || input === 'N') {
        backend.send({ type: 'permission_response', tool_use_id: state.permissionRequest.toolUseId, decision: 'deny' })
        dispatch({ type: 'PERMISSION_DISMISS' })
      }
      return
    }

    // Ctrl+C to abort or quit
    if (input === 'c' && key.ctrl) {
      if (state.isStreaming || state.isWaiting) {
        backend.send({ type: 'abort_query' })
      } else {
        backend.destroy()
      }
      return
    }

    // Enter to submit
    if (key.return) {
      submit()
      return
    }

    // Backspace
    if (key.backspace || key.delete) {
      setInputText(prev => prev.slice(0, -1))
      return
    }

    // Regular character input
    if (input && !key.ctrl && !key.meta) {
      setInputText(prev => prev + input)
    }
  })

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <Box flexDirection="column" width="100%">
      {/* Header */}
      <Box paddingX={1}>
        <Text bold color="ansi:cyan">cc-rust</Text>
        <Text dim>{' | '}{state.model || 'connecting...'}</Text>
        {state.usage.costUsd > 0 && (
          <Text dim>{' | $'}{state.usage.costUsd.toFixed(4)}</Text>
        )}
      </Box>

      {/* Messages */}
      <Box flexDirection="column" flexGrow={1} paddingX={1}>
        {state.messages.map((msg) => (
          <Box key={msg.id} marginBottom={0}>
            <Text>
              {msg.role === 'user' && <Text color="ansi:green" bold>{'> '}</Text>}
              {msg.role === 'assistant' && <Text color="ansi:blue" bold>{'< '}</Text>}
              {msg.role === 'system' && <Text color="ansi:yellow">{'* '}</Text>}
              {msg.role === 'tool_use' && <Text color="ansi:magenta">{'~ '}</Text>}
              {msg.role === 'tool_result' && <Text dim>{'  '}</Text>}
              <Text>{msg.content}</Text>
            </Text>
          </Box>
        ))}

        {/* Streaming output */}
        {state.isStreaming && state.streamingText && (
          <Box>
            <Text color="ansi:blue" bold>{'< '}</Text>
            <Text>{state.streamingText}</Text>
            <Text dim>{'_'}</Text>
          </Box>
        )}

        {/* Waiting indicator */}
        {state.isWaiting && (
          <Text dim>Thinking...</Text>
        )}
      </Box>

      {/* Permission prompt */}
      {state.permissionRequest && (
        <Box borderStyle="round" borderColor="ansi:yellow" paddingX={1}>
          <Text>
            <Text color="ansi:yellow" bold>Permission: </Text>
            <Text>{state.permissionRequest.tool}</Text>
            <Text dim>{' \u2014 '}{state.permissionRequest.command}</Text>
            <Text color="ansi:yellow">{' [y/n]'}</Text>
          </Text>
        </Box>
      )}

      {/* Input line */}
      <Box borderStyle="single" borderColor={state.isStreaming ? 'ansi:white' : 'ansi:green'} paddingX={1}>
        <Text color="ansi:green" bold>{'> '}</Text>
        <Text>{inputText}</Text>
        <Text dim>{'|'}</Text>
      </Box>
    </Box>
  )
}
