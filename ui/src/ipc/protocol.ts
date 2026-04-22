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

/**
 * Payload for an LSP plugin recommendation prompt. Mirrors Rust
 * `LspRecommendationPayload` — `request_id` correlates the frontend's
 * reply back to the originating backend request, and `plugin_name` +
 * `file_extension` populate the upstream `LspRecommendationMenu`
 * template.
 */
export interface LspRecommendationPayload {
  request_id: string
  plugin_name: string
  plugin_description?: string
  file_extension: string
  language_id?: string
}

/**
 * Snapshot of persisted "never recommend" / "disable all" preferences
 * the backend replies with after every
 * `LspCommand::recommendation_response` and also in response to
 * `LspCommand::query_settings`. Used by an LSP settings view.
 */
export interface LspRecommendationSettings {
  disabled: boolean
  muted_plugins: string[]
}

/**
 * Decision values the frontend sends in
 * `LspCommand::recommendation_response`. Matches the upstream
 * `LspRecommendationMenu` option keys.
 */
export type LspRecommendationDecision = 'yes' | 'no' | 'never' | 'disable'

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

/**
 * Scope where a config entry lives. User/project are editable; plugin/ide
 * are read-only sources managed through their own subsystems.
 */
export type ConfigScope =
  | { kind: 'user' }
  | { kind: 'project' }
  | { kind: 'plugin'; id: string }
  | { kind: 'ide'; id: string }

/**
 * Editable MCP server config entry — carries the full settings payload
 * plus its source scope. Used by the `/mcp` editor view.
 */
export interface McpServerConfigEntry {
  name: string
  transport: string
  command?: string
  args?: string[]
  url?: string
  headers?: Record<string, string>
  env?: Record<string, string>
  browser_mcp?: boolean
  scope: ConfigScope
}

/**
 * Detected IDE integration and its connection state. Drives the `/ide`
 * command's detect → select → connect flow.
 */
export interface IdeInfo {
  id: string
  name: string
  installed: boolean
  running: boolean
  selected: boolean
  connection_state?: string
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

// ---------------------------------------------------------------------------
// Agent-definition settings (mirror Rust `AgentDefinitionEntry` /
// `AgentDefinitionSource` in `ipc/subsystem_types.rs`)
// ---------------------------------------------------------------------------

export type AgentDefinitionSource =
  | { kind: 'builtin' }
  | { kind: 'user' }
  | { kind: 'project' }
  | { kind: 'plugin'; id: string }

/** Matches Rust `AgentPermissionMode` (camelCase). */
export type AgentPermissionMode =
  | 'default'
  | 'acceptEdits'
  | 'bypassPermissions'
  | 'plan'

/** Matches Rust `AgentMemoryScope` (lowercase). */
export type AgentMemoryScope = 'user' | 'project' | 'local'

export interface AgentDefinitionEntry {
  name: string
  description: string
  system_prompt: string
  tools: string[]
  disallowed_tools?: string[]
  model?: string
  color?: string
  permission_mode?: AgentPermissionMode
  memory?: AgentMemoryScope
  max_turns?: number
  effort?: string
  background?: boolean
  isolation?: string
  skills?: string[]
  hooks?: unknown
  mcp_servers?: unknown[]
  initial_prompt?: string
  filename?: string
  source: AgentDefinitionSource
  file_path?: string
}

export interface AgentToolInfo {
  name: string
  /** `"read_only" | "edit" | "execution" | "mcp" | "other"`. */
  category: string
  description?: string
  mcp_server?: string
}

export type AgentSettingsEvent =
  | { kind: 'list'; entries: AgentDefinitionEntry[] }
  | { kind: 'changed'; name: string; entry?: AgentDefinitionEntry }
  | { kind: 'error'; name: string; error: string }
  | { kind: 'tool_list'; tools: AgentToolInfo[] }
  | { kind: 'editor_opened'; file_path: string }
  | { kind: 'generate_started' }
  | {
      kind: 'generated'
      identifier: string
      when_to_use: string
      system_prompt: string
    }

export interface SubsystemStatusSnapshot {
  lsp: LspServerInfo[]
  mcp: McpServerStatusInfo[]
  plugins: PluginInfo[]
  skills: SkillInfo[]
  /**
   * Detected IDE integrations. Older snapshots may omit this field; treat
   * missing values as an empty list.
   */
  ides?: IdeInfo[]
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
  /**
   * Recommendation prompt — payload fields are flattened alongside
   * `kind` because Rust emits it with `#[serde(flatten)]`.
   */
  | ({ kind: 'recommendation_request' } & LspRecommendationPayload)
  /** Reply carrying the persisted recommendation settings. */
  | { kind: 'settings_snapshot'; settings: LspRecommendationSettings }

export type McpEvent =
  | { kind: 'server_state_changed'; server_name: string; state: string; error?: string }
  | { kind: 'tools_discovered'; server_name: string; tools: McpToolInfo[] }
  | { kind: 'resources_discovered'; server_name: string; resources: McpResourceInfo[] }
  | { kind: 'channel_notification'; server_name: string; content: string; meta: any }
  | { kind: 'server_list'; servers: McpServerStatusInfo[] }
  /** Editable config list — distinct from `server_list` (live state). */
  | { kind: 'config_list'; entries: McpServerConfigEntry[] }
  /** A config entry was upserted (entry present) or removed (entry undefined). */
  | { kind: 'config_changed'; server_name: string; entry?: McpServerConfigEntry }
  /** Config validation or persistence failure. */
  | { kind: 'config_error'; server_name: string; error: string }

export type PluginEvent =
  | { kind: 'status_changed'; plugin_id: string; name: string; status: string; error?: string }
  | { kind: 'plugin_list'; plugins: PluginInfo[] }
  /** Disk state diverged from the in-memory registry — run `/reload-plugins`. */
  | { kind: 'refresh_needed'; reason: string }
  /** Emitted after a reload cycle completes. */
  | { kind: 'reloaded'; count: number; had_error: boolean }

export type SkillEvent =
  | { kind: 'skills_loaded'; count: number }
  | { kind: 'skill_list'; skills: SkillInfo[] }

export type IdeEvent =
  | { kind: 'ide_list'; ides: IdeInfo[] }
  | { kind: 'selection_changed'; ide_id?: string }
  | { kind: 'connection_state_changed'; ide_id: string; state: string; error?: string }

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
  | {
      type: 'lsp_command'
      command:
        | { kind: 'start_server' | 'stop_server' | 'restart_server'; language_id: string }
        | { kind: 'query_status' }
        | { kind: 'query_settings' }
        | {
            kind: 'recommendation_response'
            request_id: string
            plugin_name: string
            decision: LspRecommendationDecision
          }
        | { kind: 'unmute_plugin'; plugin_name: string }
        | { kind: 'set_recommendations_disabled'; disabled: boolean }
    }
  | {
      type: 'mcp_command'
      command:
        | { kind: 'connect_server' | 'disconnect_server' | 'reconnect_server'; server_name: string }
        | { kind: 'query_status' }
        | { kind: 'query_config' }
        | { kind: 'upsert_config'; entry: McpServerConfigEntry }
        | { kind: 'remove_config'; server_name: string; scope: ConfigScope }
    }
  | {
      type: 'plugin_command'
      command:
        | { kind: 'enable' | 'disable'; plugin_id: string }
        | { kind: 'query_status' }
        | { kind: 'reload' }
        | { kind: 'uninstall'; plugin_id: string; purge_cache?: boolean }
    }
  | { type: 'skill_command'; command: { kind: 'reload' } | { kind: 'query_status' } }
  | {
      type: 'ide_command'
      command:
        | { kind: 'detect' }
        | { kind: 'select'; ide_id: string }
        | { kind: 'clear' }
        | { kind: 'reconnect' }
        | { kind: 'query_status' }
    }
  | {
      type: 'agent_settings_command'
      command:
        | { kind: 'query_list' }
        | { kind: 'upsert'; entry: AgentDefinitionEntry }
        | { kind: 'delete'; name: string; source: AgentDefinitionSource }
        | { kind: 'query_tools' }
        | { kind: 'open_in_editor'; file_path: string }
        | { kind: 'generate'; user_prompt: string; existing_names?: string[] }
    }
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
  | { type: 'ide_event'; event: IdeEvent }
  | { type: 'agent_settings_event'; event: AgentSettingsEvent }
  | { type: 'subsystem_status'; status: SubsystemStatusSnapshot }
