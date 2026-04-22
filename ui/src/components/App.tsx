import React, { useEffect, useRef, useState } from 'react'
import { useOnResize, useRenderer, useTerminalDimensions } from '@opentui/react'
import { useBackend } from '../ipc/context.js'
import type { BackendMessage, FrontendContentBlock } from '../ipc/protocol.js'
import type { KeybindingConfig } from '../keybindings.js'
import { shortcutLabel, transcriptTitle } from '../keybindings.js'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import { conversationToRawMessage } from '../store/message-model.js'
import { c } from '../theme.js'
import { AgentsDialog } from './agent-settings/index.js'
import { AgentTreePanel } from './AgentTreePanel.js'
import { InputPrompt } from './InputPrompt.js'
import { MessageList } from './MessageList.js'
import { PermissionRequestDialog } from './permissions/index.js'
import { StatusLine } from './StatusLine/index.js'
import { SubsystemStatus } from './SubsystemStatus.js'
import { Suggestions } from './Suggestions.js'
import { TeamPanel } from './TeamPanel.js'
import { WelcomeScreen } from './WelcomeScreen.js'
import { notifyBackendResize, repaintAfterResize } from './resize-sync.js'

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

function composerHint(isTranscript: boolean, keybindingConfig: KeybindingConfig | null): string {
  if (isTranscript) {
    return transcriptTitle(keybindingConfig)
  }

  return [
    `${shortcutLabel('app:toggleTranscript', { context: 'Global', config: keybindingConfig })} transcript`,
    `${shortcutLabel('app:toggleVim', { context: 'Global', config: keybindingConfig })} vim`,
    `${shortcutLabel('app:exit', { context: 'Global', config: keybindingConfig })} quit`,
  ].join(' | ')
}

export function App() {
  const backend = useBackend()
  const state = useAppState()
  const dispatch = useAppDispatch()
  const renderer = useRenderer()
  const [activePane, setActivePane] = useState<ActivePane>('input')
  const [inputStatus, setInputStatus] = useState('')
  const lastPromptPaneRef = useRef<ActivePane>('input')
  const { width: termWidth, height: termHeight } = useTerminalDimensions()
  const welcomePromptWidth = Math.min(84, Math.max(40, termWidth - 8))

  useEffect(() => {
    notifyBackendResize(backend, termWidth, termHeight)
  }, [backend, termHeight, termWidth])

  useOnResize((width, height) => {
    notifyBackendResize(backend, width, height)
    repaintAfterResize(renderer)
  })

  useEffect(() => {
    const handler = (msg: BackendMessage) => {
      switch (msg.type) {
        case 'ready':
          dispatch({
            type: 'READY',
            model: msg.model,
            sessionId: msg.session_id,
            cwd: msg.cwd,
            editorMode: (msg as BackendMessage & { editor_mode?: string | null }).editor_mode,
            viewMode: (msg as BackendMessage & { view_mode?: 'prompt' | 'transcript' | null }).view_mode,
            keybindings: (msg as BackendMessage & { keybindings?: KeybindingConfig | null }).keybindings,
          })
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
        case 'question_request':
          dispatch({ type: 'QUESTION_REQUEST', question: { id: msg.id, text: msg.text } })
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

        // ── Agent events ──────────────────────────────────────
        case 'agent_event': {
          const evt = msg.event
          switch (evt.kind) {
            case 'spawned':
              dispatch({ type: 'AGENT_SPAWNED', agentId: evt.agent_id, description: evt.description, parentAgentId: evt.parent_agent_id, agentType: evt.agent_type, model: evt.model, isBackground: evt.is_background, depth: evt.depth })
              break
            case 'completed':
              dispatch({ type: 'AGENT_COMPLETED', agentId: evt.agent_id, resultPreview: evt.result_preview, hadError: evt.had_error, durationMs: evt.duration_ms })
              break
            case 'error':
              dispatch({ type: 'AGENT_ERROR', agentId: evt.agent_id, error: evt.error, durationMs: evt.duration_ms })
              break
            case 'aborted':
              dispatch({ type: 'AGENT_ABORTED', agentId: evt.agent_id })
              break
            case 'stream_delta':
              dispatch({ type: 'AGENT_STREAM_DELTA', agentId: evt.agent_id, text: evt.text })
              break
            case 'thinking_delta':
              dispatch({ type: 'AGENT_THINKING_DELTA', agentId: evt.agent_id, thinking: evt.thinking })
              break
            case 'tree_snapshot':
              dispatch({ type: 'AGENT_TREE_SNAPSHOT', roots: evt.roots })
              break
          }
          break
        }

        // ── Team events ───────────────────────────────────────
        case 'team_event': {
          const evt = msg.event
          switch (evt.kind) {
            case 'member_joined':
              dispatch({ type: 'TEAM_MEMBER_JOINED', teamName: evt.team_name, agentId: evt.agent_id, agentName: evt.agent_name, role: evt.role })
              break
            case 'member_left':
              dispatch({ type: 'TEAM_MEMBER_LEFT', teamName: evt.team_name, agentId: evt.agent_id })
              break
            case 'message_routed':
              dispatch({ type: 'TEAM_MESSAGE_ROUTED', teamName: evt.team_name, from: evt.from, to: evt.to, summary: evt.summary ?? evt.text.slice(0, 80), timestamp: evt.timestamp })
              break
            case 'status_snapshot':
              dispatch({ type: 'TEAM_STATUS_SNAPSHOT', teamName: evt.team_name, members: evt.members, pendingMessages: evt.pending_messages })
              break
          }
          break
        }

        // ── Subsystem events ──────────────────────────────────
        case 'lsp_event':
          if (msg.event.kind === 'server_state_changed') {
            dispatch({ type: 'LSP_SERVER_STATE', languageId: msg.event.language_id, state: msg.event.state, error: msg.event.error })
          }
          break
        case 'mcp_event':
          if (msg.event.kind === 'server_state_changed') {
            dispatch({ type: 'MCP_SERVER_STATE', serverName: msg.event.server_name, state: msg.event.state, error: msg.event.error })
          }
          break
        case 'plugin_event':
          if (msg.event.kind === 'status_changed') {
            dispatch({ type: 'PLUGIN_STATUS', pluginId: msg.event.plugin_id, name: msg.event.name, status: msg.event.status, error: msg.event.error })
          }
          break
        case 'skill_event':
          if (msg.event.kind === 'skills_loaded') {
            dispatch({ type: 'SKILLS_LOADED', count: msg.event.count })
          }
          break
        case 'subsystem_status':
          dispatch({ type: 'SUBSYSTEM_STATUS', lsp: msg.status.lsp, mcp: msg.status.mcp, plugins: msg.status.plugins, skills: msg.status.skills })
          break
        case 'agent_settings_event': {
          const evt = msg.event
          switch (evt.kind) {
            case 'list':
              dispatch({ type: 'AGENT_SETTINGS_LIST', entries: evt.entries })
              break
            case 'changed':
              dispatch({ type: 'AGENT_SETTINGS_CHANGED', name: evt.name, entry: evt.entry })
              break
            case 'error':
              dispatch({ type: 'AGENT_SETTINGS_ERROR', name: evt.name, error: evt.error })
              break
          }
          break
        }
        case 'status_line_update':
          dispatch({
            type: 'CUSTOM_STATUS_LINE_UPDATE',
            lines: msg.lines,
            error: msg.error,
            updatedAt: Date.now(),
          })
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
    ? composerHint(true, state.keybindingConfig)
    : `${inputStatus || composerHint(false, state.keybindingConfig)}${queueSuffix}`

  return (
    <box flexDirection="column" width="100%" height="100%" backgroundColor={c.bg}>
      {!isWelcome && (
        <StatusLine
          cwd={state.cwd}
          model={state.model}
          usage={state.usage}
          vimMode={state.vimEnabled ? state.vimMode : undefined}
          viewMode={state.viewMode}
        />
      )}
      {isWelcome ? (
        <box
          flexGrow={1}
          flexDirection="column"
          alignItems="center"
          justifyContent="center"
          width="100%"
          backgroundColor={c.bg}
        >
          <WelcomeScreen />
          <box
            marginTop={1}
            width={welcomePromptWidth}
            border
            borderStyle="rounded"
            borderColor="#45475A"
            backgroundColor={c.bg}
            paddingX={1}
            title={composerHint(false, state.keybindingConfig)}
            titleAlignment="right"
          >
            <InputPrompt isActive onActivate={() => setActivePane('input')} viewMode="prompt" />
          </box>
          <box marginTop={1}>
            <text fg="#45475A">
              <em>{shortcutLabel('app:exit', { context: 'Global', config: state.keybindingConfig })} to quit</em>
            </text>
          </box>
        </box>
      ) : (
        <box flexGrow={1} flexDirection="column" backgroundColor={c.bg}>
          <box flexGrow={1}>
            <MessageList
              isActive={isTranscript || activePane === 'messages'}
              onActivate={() => setActivePane('messages')}
              viewMode={state.viewMode}
            >
              {state.suggestions.length > 0 && !isBusy && state.viewMode === 'prompt' && <Suggestions />}
            </MessageList>
          </box>
          <AgentTreePanel />
          <TeamPanel />
          <SubsystemStatus />
          <box
            width="100%"
            border
            borderStyle="rounded"
            borderColor="#45475A"
            backgroundColor={c.bg}
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
      {state.permissionRequest && <PermissionRequestDialog request={state.permissionRequest} />}
      <AgentsDialog />
    </box>
  )
}
