import { useChatStore } from './store'
import type { AppState, StreamingBlock } from './types'

const API_BASE = ''  // same origin in dev (Vite proxy) and prod (embedded)

// ---------------------------------------------------------------------------
// Connection health monitoring
// ---------------------------------------------------------------------------

let connectionHealthy = true

/** Check if the backend server is reachable */
export async function checkConnection(): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/api/state`, { signal: AbortSignal.timeout(3000) })
    connectionHealthy = res.ok
    return connectionHealthy
  } catch {
    connectionHealthy = false
    return false
  }
}

/** Get current connection status */
export function isConnected(): boolean {
  return connectionHealthy
}

/**
 * Send a chat message and consume the SSE stream.
 */
export async function sendChatMessage(message: string, sessionId?: string): Promise<void> {
  const store = useChatStore.getState()
  store.addUserMessage(message)
  store.startStreaming()

  try {
    const response = await fetch(`${API_BASE}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        message,
        session_id: sessionId,
      }),
    })

    if (!response.ok) {
      const err = await response.json().catch(() => ({ error: response.statusText }))
      throw new Error(err.error || `HTTP ${response.status}`)
    }

    if (!response.body) {
      throw new Error('No response body')
    }

    await consumeSSEStream(response.body)
  } catch (error) {
    const errMsg = error instanceof Error ? error.message : 'Unknown error'
    store.finishStreaming()
    store.addAssistantMessage({
      id: crypto.randomUUID(),
      role: 'system',
      content: `Error: ${errMsg}`,
      timestamp: Date.now(),
    })
  }
}

/**
 * Parse and dispatch SSE events from a ReadableStream.
 */
async function consumeSSEStream(body: ReadableStream<Uint8Array>): Promise<void> {
  const reader = body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''

  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break

      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() || '' // keep incomplete line in buffer

      let currentEvent = ''
      let currentData = ''

      for (const line of lines) {
        if (line.startsWith('event: ')) {
          currentEvent = line.slice(7).trim()
        } else if (line.startsWith('data: ')) {
          currentData = line.slice(6)
        } else if (line === '' && currentEvent && currentData) {
          // End of event block
          dispatchSSEEvent(currentEvent, currentData)
          currentEvent = ''
          currentData = ''
        }
      }
    }
  } finally {
    reader.releaseLock()
    // Ensure streaming state is cleaned up
    const latestStore = useChatStore.getState()
    if (latestStore.isStreaming) {
      latestStore.finishStreaming()
    }
  }
}

/**
 * Dispatch a single parsed SSE event to the store.
 */
function dispatchSSEEvent(event: string, dataStr: string): void {
  const store = useChatStore.getState()

  // Always log raw events
  store.addRawEvent(event, dataStr)

  try {
    const data = JSON.parse(dataStr)

    switch (event) {
      case 'stream_event': {
        handleStreamEvent(data, store)
        break
      }

      case 'assistant': {
        const msg = data.message || data
        const textContent = (msg.content || [])
          .filter((b: any) => b.type === 'text')
          .map((b: any) => b.text)
          .join('')

        store.addAssistantMessage({
          id: msg.uuid || crypto.randomUUID(),
          role: 'assistant',
          content: textContent || store.streamingContent,
          timestamp: msg.timestamp || Date.now(),
          contentBlocks: msg.content,
          usage: msg.usage,
          costUsd: msg.cost_usd,
          toolCalls: (msg.content || []).filter((b: any) => b.type === 'tool_use'),
        })
        break
      }

      case 'user_replay': {
        // Replay contains the tool_result blocks that answer the preceding
        // assistant's tool_use calls. Stitch them onto the last assistant
        // message so ToolCallCard can pair tool_use with its result (which
        // is how screenshots/pages/console output become visible).
        const blocks = (data.content_blocks || data.content || []) as any[]
        const toolResults = blocks.filter(b => b && b.type === 'tool_result')
        if (toolResults.length > 0) {
          store.appendToolResultsToLastAssistant(toolResults)
        }
        break
      }

      case 'result': {
        store.finishStreaming(data)
        break
      }

      case 'api_retry': {
        store.addAssistantMessage({
          id: crypto.randomUUID(),
          role: 'system',
          content: `API retry ${data.attempt}/${data.max_retries}: ${data.error}`,
          timestamp: Date.now(),
        })
        break
      }

      case 'tool_use_summary': {
        // Show summary as a system message
        if (data.summary) {
          store.addAssistantMessage({
            id: crypto.randomUUID(),
            role: 'system',
            content: data.summary,
            timestamp: Date.now(),
          })
        }
        break
      }

      case 'system_init': {
        store.setAppState({
          model: data.model,
          session_id: data.session_id,
          tools: data.tools,
          permission_mode: data.permission_mode,
          thinking_enabled: null,
          fast_mode: false,
          effort: null,
        })
        break
      }

      default:
        // Unknown event type — logged via addRawEvent above
        break
    }
  } catch (e) {
    console.error('Failed to parse SSE event:', event, dataStr, e)
  }
}

/**
 * Handle stream_event SSE events with multi-block tracking.
 *
 * Stream events follow this sequence per API response:
 *   message_start → (content_block_start → content_block_delta* → content_block_stop)+ → message_delta → message_stop
 */
function handleStreamEvent(data: any, store: ReturnType<typeof useChatStore.getState>): void {
  const evt = data.event
  if (!evt) return

  switch (evt.type) {
    case 'content_block_start': {
      const idx = evt.index ?? 0
      const block = evt.content_block
      if (!block) break

      const streamBlock: StreamingBlock = {
        index: idx,
        type: block.type || 'text',
        content: '',
        done: false,
      }

      if (block.type === 'tool_use') {
        streamBlock.toolName = block.name
        streamBlock.toolId = block.id
        streamBlock.toolInput = ''
      } else if (block.type === 'thinking') {
        streamBlock.content = block.thinking || ''
      } else if (block.type === 'text') {
        streamBlock.content = block.text || ''
      }

      store.startStreamingBlock(idx, streamBlock)
      break
    }

    case 'content_block_delta': {
      const idx = evt.index ?? 0
      const delta = evt.delta

      if (!delta) break

      if (delta.type === 'text_delta' && delta.text) {
        store.appendToStreamingBlock(idx, delta.text)
        // Also append to flat streamingContent for backward compat
        store.appendStreamContent(delta.text)
      } else if (delta.type === 'thinking_delta' && delta.thinking) {
        store.appendToStreamingBlock(idx, delta.thinking)
      } else if (delta.type === 'input_json_delta' && delta.partial_json) {
        store.appendToolInputDelta(idx, delta.partial_json)
      } else if (delta.text) {
        // Fallback: some providers send raw text without wrapping
        store.appendToStreamingBlock(idx, delta.text)
        store.appendStreamContent(delta.text)
      }
      break
    }

    case 'content_block_stop': {
      const idx = evt.index ?? 0
      store.finishStreamingBlock(idx)
      break
    }

    case 'message_start':
    case 'message_delta':
    case 'message_stop':
      // These are lifecycle markers; no action needed for streaming display
      break
  }
}

/**
 * Abort the current generation.
 */
export async function abortChat(sessionId?: string): Promise<void> {
  await fetch(`${API_BASE}/api/abort`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ session_id: sessionId }),
  })
}

/**
 * Fetch the current application state.
 */
export async function fetchAppState(): Promise<AppState> {
  const res = await fetch(`${API_BASE}/api/state`)
  return res.json()
}

// ---------------------------------------------------------------------------
// Phase 3: Settings and Command APIs
// ---------------------------------------------------------------------------

interface SettingsResponse {
  ok: boolean
  message: string
}

/**
 * Update a setting on the server.
 */
export async function updateSetting(action: string, value: unknown): Promise<SettingsResponse> {
  const res = await fetch(`${API_BASE}/api/settings`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ action, value }),
  })
  const data = await res.json()

  // Refresh app state after successful mutation
  if (data.ok) {
    const newState = await fetchAppState()
    useChatStore.getState().setAppState(newState)
  }

  return data
}

interface CommandResponse {
  type: 'output' | 'clear' | 'error'
  content: string
}

/**
 * Execute a slash command on the server.
 */
export async function executeCommand(command: string, args: string = ''): Promise<CommandResponse> {
  const res = await fetch(`${API_BASE}/api/command`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ command, args }),
  })
  const data: CommandResponse = await res.json()

  // Handle clear command
  if (data.type === 'clear') {
    useChatStore.getState().clearMessages()
  }

  // Show output as system message
  if (data.content && data.type !== 'clear') {
    useChatStore.getState().addAssistantMessage({
      id: crypto.randomUUID(),
      role: 'system',
      content: `/${command}${args ? ' ' + args : ''}: ${data.content}`,
      timestamp: Date.now(),
    })
  }

  // Refresh app state (command may have changed model, permissions, etc.)
  try {
    const newState = await fetchAppState()
    useChatStore.getState().setAppState(newState)
  } catch { /* ignore */ }

  return data
}
