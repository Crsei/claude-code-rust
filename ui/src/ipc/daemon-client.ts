/**
 * DaemonClient -- HTTP/SSE transport for connecting ink-terminal to the
 * KAIROS daemon.
 *
 * Unlike {@link RustBackend} (JSONL over stdio), this client communicates
 * with a long-running daemon process via HTTP endpoints and Server-Sent
 * Events on `127.0.0.1`.
 *
 * Endpoints consumed (see `src/daemon/routes.rs` + `src/daemon/sse.rs`):
 *   POST /api/attach      -- register client, receive missed events
 *   POST /api/detach      -- unregister client
 *   POST /api/submit      -- submit user prompt
 *   POST /api/abort       -- abort running query
 *   POST /api/command     -- execute slash command
 *   POST /api/permission  -- respond to permission request
 *   GET  /events          -- SSE stream (named events)
 *   GET  /health          -- liveness probe
 */

import { EventEmitter } from 'events'
import type { BackendMessage } from './protocol.js'

const DEFAULT_PORT = 19836

/**
 * Represents a single SSE event as stored in the daemon's ring buffer and
 * returned by `/api/attach` in the `missed_events` array.
 */
interface SseEvent {
  id: string
  event_type: string
  data: any
}

export class DaemonClient extends EventEmitter {
  private baseUrl: string
  private eventSource: EventSource | null = null
  private clientId: string
  private onMessageCallback: ((msg: BackendMessage) => void) | null = null
  private lastEventId: string | undefined

  constructor(port: number = DEFAULT_PORT) {
    super()
    this.baseUrl = `http://127.0.0.1:${port}`
    this.clientId = `ui_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
  }

  // ---------------------------------------------------------------------------
  // Connection lifecycle
  // ---------------------------------------------------------------------------

  /**
   * Attach to the daemon and open an SSE stream.
   *
   * 1. POST `/api/attach` with our client id (and optional last-seen event id)
   *    to register and retrieve any events we missed while disconnected.
   * 2. Open an `EventSource` on `/events` for live streaming.
   */
  async connect(lastEventId?: string): Promise<void> {
    this.lastEventId = lastEventId

    // -- Step 1: Attach and replay missed events --
    const attachRes = await fetch(`${this.baseUrl}/api/attach`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        client_id: this.clientId,
        last_seen_event: lastEventId ?? null,
      }),
    })

    if (!attachRes.ok) {
      throw new Error(`Failed to attach to daemon: HTTP ${attachRes.status}`)
    }

    const { missed_events } = (await attachRes.json()) as {
      missed_events: SseEvent[]
    }

    for (const event of missed_events) {
      this.lastEventId = event.id
      this.dispatchEvent(event.event_type, event.data)
    }

    // -- Step 2: Open SSE stream --
    const params = new URLSearchParams({ client_id: this.clientId })
    if (this.lastEventId) {
      params.set('last_event_id', this.lastEventId)
    }

    // The SSE endpoint lives at /events (not /api/events).
    const url = `${this.baseUrl}/events?${params.toString()}`
    this.eventSource = new EventSource(url)

    // The daemon sends named events (event_type field maps to SSE `event:`
    // lines).  We listen for the known event types so that `onmessage` (which
    // only fires for unnamed events) is not our sole hook.
    const knownEvents = [
      'stream_start',
      'stream_delta',
      'stream_end',
      'assistant_message',
      'user_replay',
      'permission_request',
      'system_info',
      'usage_update',
      'suggestions',
      'error',
    ]

    for (const eventType of knownEvents) {
      this.eventSource.addEventListener(eventType, (ev: MessageEvent) => {
        try {
          const data = JSON.parse(ev.data)
          if (ev.lastEventId) this.lastEventId = ev.lastEventId
          this.dispatchEvent(eventType, data)
        } catch {
          // ignore malformed JSON
        }
      })
    }

    // Fallback for unnamed events.
    this.eventSource.onmessage = (ev: MessageEvent) => {
      try {
        const data = JSON.parse(ev.data)
        if (ev.lastEventId) this.lastEventId = ev.lastEventId
        this.onMessageCallback?.(data)
        this.emit('message', data)
      } catch {
        // ignore
      }
    }

    this.eventSource.onerror = () => {
      this.emit('error', new Error('SSE connection lost'))
    }
  }

  /**
   * Gracefully disconnect: close the SSE stream and POST `/api/detach`.
   */
  async disconnect(): Promise<void> {
    this.eventSource?.close()
    this.eventSource = null

    try {
      await fetch(`${this.baseUrl}/api/detach`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ client_id: this.clientId }),
      })
    } catch {
      // Best-effort; daemon may already be gone.
    }
  }

  // ---------------------------------------------------------------------------
  // Commands (mirror FrontendMessage variants)
  // ---------------------------------------------------------------------------

  /** Submit a user prompt. Equivalent to `{ type: 'submit_prompt' }`. */
  async submit(text: string, id: string): Promise<void> {
    const res = await fetch(`${this.baseUrl}/api/submit`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text, id }),
    })
    if (!res.ok) {
      throw new Error(`submit failed: HTTP ${res.status}`)
    }
  }

  /** Abort the currently running query. */
  async abort(): Promise<void> {
    await fetch(`${this.baseUrl}/api/abort`, { method: 'POST' })
  }

  /** Execute a slash command. Equivalent to `{ type: 'slash_command' }`. */
  async command(raw: string): Promise<void> {
    await fetch(`${this.baseUrl}/api/command`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ raw }),
    })
  }

  /** Respond to a permission request. */
  async permission(toolUseId: string, decision: string): Promise<void> {
    await fetch(`${this.baseUrl}/api/permission`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tool_use_id: toolUseId, decision }),
    })
  }

  // ---------------------------------------------------------------------------
  // Health
  // ---------------------------------------------------------------------------

  /** Check daemon liveness. Returns `true` if `/health` responds 200. */
  async isAlive(): Promise<boolean> {
    try {
      const res = await fetch(`${this.baseUrl}/health`)
      return res.ok
    } catch {
      return false
    }
  }

  // ---------------------------------------------------------------------------
  // Callbacks
  // ---------------------------------------------------------------------------

  /** Register a callback for all backend messages. */
  onMessage(callback: (msg: BackendMessage) => void): void {
    this.onMessageCallback = callback
  }

  /** Current client identifier (sent to the daemon on attach/detach). */
  getClientId(): string {
    return this.clientId
  }

  /** Last seen SSE event id (for reconnection). */
  getLastEventId(): string | undefined {
    return this.lastEventId
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  /**
   * Route a named SSE event to both the callback and the EventEmitter.
   * The `event_type` from the daemon maps to `BackendMessage.type`.
   */
  private dispatchEvent(eventType: string, data: any): void {
    const msg = { type: eventType, ...data } as BackendMessage
    this.onMessageCallback?.(msg)
    this.emit('message', msg)
  }
}
