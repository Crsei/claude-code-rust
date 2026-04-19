// SdkMessage types — mirrors Rust engine/sdk_types.rs

export interface Usage {
  input_tokens: number
  output_tokens: number
  cache_read_input_tokens: number
  cache_creation_input_tokens: number
}

export interface UsageTracking {
  total_input_tokens: number
  total_output_tokens: number
  total_cache_read_tokens: number
  total_cache_creation_tokens: number
  total_cost_usd: number
  api_call_count: number
}

/**
 * A structured sub-block inside a tool_result. Mirrors the shape produced by
 * `ToolResultContent::Blocks(...)` on the Rust side — specifically supports
 * images (base64) so that browser-MCP screenshots render inline in the UI.
 */
export interface ToolResultInnerBlock {
  type: 'text' | 'image'
  // text
  text?: string
  // image
  source?: ImageSource
}

/** Base64-encoded image source, as used by the Anthropic API. */
export interface ImageSource {
  type: 'base64'
  media_type: string
  data: string
}

export interface ContentBlock {
  type: 'text' | 'tool_use' | 'tool_result' | 'thinking' | 'redacted_thinking' | 'image'
  // text
  text?: string
  // tool_use
  id?: string
  name?: string
  input?: Record<string, unknown>
  // tool_result
  tool_use_id?: string
  content?: string | ToolResultInnerBlock[]
  is_error?: boolean
  // image
  source?: ImageSource
  // thinking
  thinking?: string
  signature?: string
}

export interface AssistantMessage {
  uuid: string
  timestamp: number
  role: string
  content: ContentBlock[]
  usage: Usage | null
  stop_reason: string | null
  cost_usd: number
}

// SSE event types
export type SseEventType =
  | 'system_init'
  | 'assistant'
  | 'user_replay'
  | 'stream_event'
  | 'compact_boundary'
  | 'api_retry'
  | 'tool_use_summary'
  | 'result'

export interface SystemInitEvent {
  type: 'system_init'
  tools: string[]
  model: string
  permission_mode: string
  session_id: string
}

export interface AssistantEvent {
  type: 'assistant'
  message: AssistantMessage
  session_id: string
  parent_tool_use_id: string | null
}

export interface StreamEventData {
  type: 'stream_event'
  event: {
    type: string
    // text delta
    content?: string
    // content block start/stop
    index?: number
    content_block?: ContentBlock
    delta?: Record<string, unknown>
  }
  session_id: string
}

export interface ApiRetryEvent {
  type: 'api_retry'
  attempt: number
  max_retries: number
  retry_delay_ms: number
  error_status: number | null
  error: string
}

export interface ToolUseSummaryEvent {
  type: 'tool_use_summary'
  summary: string
  preceding_tool_use_ids: string[]
}

export interface ResultEvent {
  type: 'result'
  subtype: 'success' | 'error_during_execution' | 'error_max_turns' | 'error_max_budget_usd'
  is_error: boolean
  duration_ms: number
  duration_api_ms: number
  num_turns: number
  result: string
  total_cost_usd: number
  usage: UsageTracking
  errors: string[]
}

export type SdkEvent =
  | SystemInitEvent
  | AssistantEvent
  | StreamEventData
  | ApiRetryEvent
  | ToolUseSummaryEvent
  | ResultEvent

// ---------------------------------------------------------------------------
// Streaming block tracking — tracks what's being streamed in real time
// ---------------------------------------------------------------------------

/** A single block being streamed — can be text, tool_use, or thinking */
export interface StreamingBlock {
  index: number
  type: 'text' | 'tool_use' | 'thinking' | 'redacted_thinking'
  content: string
  // tool_use specific
  toolName?: string
  toolId?: string
  toolInput?: string  // accumulated JSON string
  // complete?
  done: boolean
}

// ---------------------------------------------------------------------------
// UI message types for the chat store
// ---------------------------------------------------------------------------

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: number
  // Assistant-specific
  contentBlocks?: ContentBlock[]
  usage?: Usage | null
  costUsd?: number
  isStreaming?: boolean
  // Tool use
  toolCalls?: ContentBlock[]
}

export interface AppState {
  model: string
  session_id: string
  tools: string[]
  permission_mode: string
  thinking_enabled: boolean | null
  fast_mode: boolean
  effort: string | null
  // Phase 3 additions
  usage?: UsageTracking
  commands?: CommandInfo[]
}

export interface CommandInfo {
  name: string
  aliases: string[]
  description: string
}

// ---------------------------------------------------------------------------
// Session navigation (Phase 2 of the web UI overhaul)
// ---------------------------------------------------------------------------

/** Workspace descriptor — the grouping key used on the session sidebar. */
export interface WorkspaceInfo {
  key: string
  root: string
  name: string
}

/** Summary row returned by GET /api/sessions. */
export interface SessionSummary {
  session_id: string
  created_at: number
  last_modified: number
  message_count: number
  cwd: string
  title: string
  workspace_key: string
  workspace_root: string
  workspace_name: string
}

/** GET /api/sessions response shape. */
export interface SessionListResponse {
  current_workspace: WorkspaceInfo
  active_session_id: string
  sessions: SessionSummary[]
}

/** One message from a loaded session. Mirrors the Rust `StoredMessage`. */
export interface StoredMessage {
  uuid: string
  timestamp: number
  role: 'user' | 'assistant' | 'system' | 'progress' | 'attachment'
  content: string
  content_blocks?: ContentBlock[]
}

/** Full session payload returned by GET /api/sessions/:id and resume. */
export interface SessionDetail {
  session_id: string
  created_at: number
  last_modified: number
  cwd: string
  title: string
  workspace_name: string
  messages: StoredMessage[]
}
