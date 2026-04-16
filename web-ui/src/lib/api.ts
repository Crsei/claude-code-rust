import { useChatStore } from './store'
import type { AppState } from './types'

const API_BASE = ''  // same origin in dev (Vite proxy) and prod (embedded)

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
        // Handle text deltas for streaming display
        if (data.event?.type === 'content_block_delta' && data.event?.delta?.text) {
          store.appendStreamContent(data.event.delta.text)
        } else if (data.event?.type === 'content_block_delta' && data.event?.delta?.type === 'text_delta') {
          store.appendStreamContent(data.event.delta.text || '')
        }
        break
      }

      case 'assistant': {
        const msg = data.message || data
        const content = (msg.content || [])
          .filter((b: any) => b.type === 'text')
          .map((b: any) => b.text)
          .join('')

        store.addAssistantMessage({
          id: msg.uuid || crypto.randomUUID(),
          role: 'assistant',
          content: content || store.streamingContent,
          timestamp: msg.timestamp || Date.now(),
          contentBlocks: msg.content,
          usage: msg.usage,
          costUsd: msg.cost_usd,
          toolCalls: (msg.content || []).filter((b: any) => b.type === 'tool_use'),
        })
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
        // Could render inline; for now just log
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
