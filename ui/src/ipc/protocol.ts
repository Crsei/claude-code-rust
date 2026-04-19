export type FrontendContentBlock =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: any }
  | { type: 'tool_result'; tool_use_id: string; content: ToolResultContent; is_error?: boolean }
  | { type: 'thinking'; thinking: string; signature?: string | null }
  | { type: 'redacted_thinking'; data: string }
  | { type: 'image'; source: ImageSource }

export type ToolResultContent = string | FrontendContentBlock[]

/** Base64-encoded image source — mirrors Rust `ImageSource`. */
export interface ImageSource {
  type: 'base64'
  media_type: string
  data: string
}

/**
 * Structured tool result info forwarded over IPC as part of
 * `BackendMessage.tool_result.content_blocks`. Mirrors Rust
 * `ToolResultContentInfo` in `src/ipc/protocol/base.rs` — images now include
 * the raw base64 payload so browser MCP screenshots render inline in the UI.
 */
export type ToolResultContentInfo =
  | { type: 'text'; text: string }
  | { type: 'image'; media_type: string; size_bytes?: number; data?: string }

// Frontend -> Backend
export type ConversationMessage = {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: number
  content_blocks?: FrontendContentBlock[]
  cost_usd?: number
  thinking?: string
  level?: string
}

// ---------------------------------------------------------------------------
// Agent types
// ---------------------------------------------------------------------------

export interface AgentNode {
  agent_id: string
  parent_agent_id?: string
  description: string
  agent_type?: string
  model?: string
  state: 'running' | 'completed' | 'error' | 'aborted'
  is_background: boolean
  depth: number
  chain_id: string
  spawned_at: number
  completed_at?: number
  duration_ms?: number
  result_preview?: string
  had_error: boolean
  children: AgentNode[]
}

export interface TeamMemberInfo {
  agent_id: string
  agent_name: string
  role: string
  is_active: boolean
  unread_messages: number
}

// ---------------------------------------------------------------------------
// Subsystem types
// ---------------------------------------------------------------------------

export interface LspDiagnostic {
  range: { start_line: number; start_character: number; end_line: number; end_character: number }
  severity: string
  message: string
  source?: string
  code?: string
}

export interface LspServerInfo {
  language_id: string
  state: string
  extensions: string[]
  open_files_count: number
  error?: string
}

export interface McpToolInfo { name: string; description?: string }
export interface McpResourceInfo { uri: string; name?: string; mime_type?: string }
export interface McpServerStatusInfo {
  name: string
  state: string
  transport: string
  tools_count: number
  resources_count: number
  server_info?: { name: string; version: string }
  instructions?: string
  error?: string
}

export interface PluginInfo {
  id: string
  name: string
  version: string
  status: string
  contributed_tools: string[]
  contributed_skills: string[]
  contributed_mcp_servers: string[]
  error?: string
}

export interface SkillInfo {
  name: string
  source: string
  description: string
  user_invocable: boolean
  model_invocable: boolean
}

export interface SubsystemStatusSnapshot {
  lsp: LspServerInfo[]
  mcp: McpServerStatusInfo[]
  plugins: PluginInfo[]
  skills: SkillInfo[]
  timestamp: number
}

// ---------------------------------------------------------------------------
// Status line payload contract
// ---------------------------------------------------------------------------

export interface StatusLineModelInfo {
  id: string
  displayName?: string
  backend?: string
}

export interface StatusLineWorkspaceStatus {
  cwd: string
  projectDir?: string
  gitBranch?: string
  isWorktree?: boolean
  gitWorktree?: string
}

export interface StatusLineContextStatus {
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  maxTokens?: number
  usedFraction?: number
}

export interface StatusLineCostStatus {
  totalUsd: number
  apiCalls: number
  sessionDurationSecs?: number
}

export interface StatusLineVimStatus {
  mode: string
}

export interface StatusLineWorktreeStatus {
  name: string
  path: string
  branch?: string
  originalCwd: string
  originalBranch?: string
}

export interface StatusLinePayload {
  hookEventName: 'StatusLine'
  version: number
  sessionId?: string
  model?: StatusLineModelInfo
  workspace?: StatusLineWorkspaceStatus
  context?: StatusLineContextStatus
  cost?: StatusLineCostStatus
  outputStyle?: string
  vim?: StatusLineVimStatus
  worktree?: StatusLineWorktreeStatus
  streaming: boolean
  messageCount: number
}

// ---------------------------------------------------------------------------
// Agent events (Backend -> Frontend)
// ---------------------------------------------------------------------------

export type AgentEvent =
  | { kind: 'spawned'; agent_id: string; parent_agent_id?: string; description: string; agent_type?: string; model?: string; is_background: boolean; depth: number; chain_id: string }
  | { kind: 'completed'; agent_id: string; result_preview: string; had_error: boolean; duration_ms: number; output_tokens?: number }
  | { kind: 'error'; agent_id: string; error: string; duration_ms: number }
  | { kind: 'aborted'; agent_id: string }
  | { kind: 'stream_delta'; agent_id: string; text: string }
  | { kind: 'thinking_delta'; agent_id: string; thinking: string }
  | { kind: 'tool_use'; agent_id: string; tool_use_id: string; tool_name: string; input: any }
  | { kind: 'tool_result'; agent_id: string; tool_use_id: string; output: string; is_error: boolean }
  | { kind: 'tree_snapshot'; roots: AgentNode[] }

export type TeamEvent =
  | { kind: 'member_joined'; team_name: string; agent_id: string; agent_name: string; role: string }
  | { kind: 'member_left'; team_name: string; agent_id: string; agent_name: string }
  | { kind: 'message_routed'; team_name: string; from: string; to: string; text: string; timestamp: number; summary?: string }
  | { kind: 'status_snapshot'; team_name: string; members: TeamMemberInfo[]; pending_messages: number }

// ---------------------------------------------------------------------------
// Subsystem events (Backend -> Frontend)
// ---------------------------------------------------------------------------

export type LspEvent =
  | { kind: 'server_state_changed'; language_id: string; state: string; error?: string }
  | { kind: 'diagnostics_published'; uri: string; diagnostics: LspDiagnostic[] }
  | { kind: 'server_list'; servers: LspServerInfo[] }

export type McpEvent =
  | { kind: 'server_state_changed'; server_name: string; state: string; error?: string }
  | { kind: 'tools_discovered'; server_name: string; tools: McpToolInfo[] }
  | { kind: 'resources_discovered'; server_name: string; resources: McpResourceInfo[] }
  | { kind: 'channel_notification'; server_name: string; content: string; meta: any }
  | { kind: 'server_list'; servers: McpServerStatusInfo[] }

export type PluginEvent =
  | { kind: 'status_changed'; plugin_id: string; name: string; status: string; error?: string }
  | { kind: 'plugin_list'; plugins: PluginInfo[] }

export type SkillEvent =
  | { kind: 'skills_loaded'; count: number }
  | { kind: 'skill_list'; skills: SkillInfo[] }

// ---------------------------------------------------------------------------
// FrontendMessage (Frontend -> Backend)
// ---------------------------------------------------------------------------

export type FrontendMessage =
  | { type: 'submit_prompt'; text: string; id: string }
  | { type: 'abort_query' }
  | { type: 'permission_response'; tool_use_id: string; decision: string }
  | { type: 'question_response'; id: string; text: string }
  | { type: 'slash_command'; raw: string }
  | { type: 'resize'; cols: number; rows: number }
  | { type: 'quit' }
  // Agent/Team commands
  | { type: 'agent_command'; command: { kind: 'abort_agent'; agent_id: string } | { kind: 'query_active_agents' } | { kind: 'query_agent_output'; agent_id: string } }
  | { type: 'team_command'; command: { kind: 'inject_message'; team_name: string; to: string; text: string } | { kind: 'query_team_status'; team_name: string } }
  // Subsystem commands
  | { type: 'lsp_command'; command: { kind: 'start_server' | 'stop_server' | 'restart_server'; language_id: string } | { kind: 'query_status' } }
  | { type: 'mcp_command'; command: { kind: 'connect_server' | 'disconnect_server' | 'reconnect_server'; server_name: string } | { kind: 'query_status' } }
  | { type: 'plugin_command'; command: { kind: 'enable' | 'disable'; plugin_id: string } | { kind: 'query_status' } }
  | { type: 'skill_command'; command: { kind: 'reload' } | { kind: 'query_status' } }
  | { type: 'query_subsystem_status' }

// ---------------------------------------------------------------------------
// BackendMessage (Backend -> Frontend)
// ---------------------------------------------------------------------------

export type BackendMessage =
  // Core conversation
  | {
      type: 'ready'
      session_id: string
      model: string
      cwd: string
      editor_mode?: string | null
      view_mode?: 'prompt' | 'transcript' | null
      keybindings?: any
    }
  | { type: 'stream_start'; message_id: string }
  | { type: 'stream_delta'; message_id: string; text: string }
  | { type: 'thinking_delta'; message_id: string; thinking: string }
  | { type: 'stream_end'; message_id: string }
  | { type: 'assistant_message'; id: string; content: FrontendContentBlock[] | null; cost_usd: number }
  | { type: 'tool_use'; id: string; name: string; input: any }
  | {
      type: 'tool_result'
      tool_use_id: string
      output: string
      is_error: boolean
      /**
       * Structured content blocks when the result includes non-text data
       * (e.g. images from a browser MCP screenshot). Forwarded from Rust's
       * `BackendMessage::ToolResult::content_blocks`.
       */
      content_blocks?: ToolResultContentInfo[]
    }
  | { type: 'permission_request'; tool_use_id: string; tool: string; command: string; options: string[] }
  | { type: 'question_request'; id: string; text: string }
  | { type: 'system_info'; text: string; level: string }
  | { type: 'conversation_replaced'; messages: ConversationMessage[] }
  | { type: 'usage_update'; input_tokens: number; output_tokens: number; cost_usd: number }
  /**
   * Scriptable status-line snapshot — see Rust issue #11.
   *
   * `payload` mirrors the JSON Rust pipes into `statusLine.command`
   * (hookEventName/version/model/workspace/context/cost/…). Frontends
   * that want to drive their own script can pass it to their runner.
   *
   * `lines` is the already-rendered stdout (split on `\n`). Frontends
   * that trust the Rust runner can show this directly and skip spawning.
   *
   * `error` is populated when the last run failed; callers should fall
   * back to the built-in footer in that case.
   */
  | {
      type: 'status_line_update'
      payload: StatusLinePayload
      lines: string[]
      error?: string
    }
  | { type: 'suggestions'; items: string[] }
  | { type: 'background_agent_complete'; agent_id: string; description: string; result_preview: string; had_error: boolean; duration_ms: number }
  | { type: 'brief_message'; message: string; status: string; attachments: string[] }
  | { type: 'autonomous_start'; source: string; time: string }
  | { type: 'notification_sent'; title: string; level: string }
  | { type: 'error'; message: string; recoverable: boolean }
  // Agent/Team events
  | { type: 'agent_event'; event: AgentEvent }
  | { type: 'team_event'; event: TeamEvent }
  // Subsystem events
  | { type: 'lsp_event'; event: LspEvent }
  | { type: 'mcp_event'; event: McpEvent }
  | { type: 'plugin_event'; event: PluginEvent }
  | { type: 'skill_event'; event: SkillEvent }
  | { type: 'subsystem_status'; status: SubsystemStatusSnapshot }
