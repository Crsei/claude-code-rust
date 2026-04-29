import type {
  AgentDefinitionEntry,
  AgentNode,
  AgentToolInfo,
  FrontendContentBlock,
  LspDiagnostic,
  LspRecommendationPayload,
  LspRecommendationSettings,
  LspServerInfo,
  McpResourceInfo,
  McpServerConfigEntry,
  McpServerStatusInfo,
  McpToolInfo,
  PluginInfo,
  PlanWorkflowRecord,
  SkillInfo,
  TeamMemberInfo,
} from '../ipc/protocol.js'
import type { EditorMode, KeybindingConfig, ViewMode } from '../keybindings.js'
import type { RawMessage } from './message-model.js'

export interface Usage {
  inputTokens: number
  outputTokens: number
  costUsd: number
}

export interface PermissionRequest {
  toolUseId: string
  tool: string
  command: string
  options: string[]
}

export interface BackgroundAgent {
  agentId: string
  description: string
  startedAt: number
  completedAt?: number
  resultPreview?: string
  hadError?: boolean
  durationMs?: number
}

/**
 * Live-shell progress state for a Bash tool invocation. Populated by the
 * `tool_progress` IPC event the Rust backend emits while a `BashTool`
 * runs — see `crates/claude-code-rs/src/tools/exec/bash.rs`. Used by the
 * `ShellProgressMessage` component to render a "Running… (12s)" tail
 * view while the command is in-flight and the final output preview once
 * it finishes.
 */
export interface ShellProgressState {
  toolUseId: string
  tool: string
  /** Most-recent tail-capped output snapshot. */
  output: string
  /** Accumulated full output (what the UI shows in verbose mode). */
  fullOutput: string
  elapsedSeconds: number
  totalLines?: number
  totalBytes?: number
  timeoutMs?: number
  /** `true` once the corresponding `tool_result` arrives. */
  completed: boolean
  updatedAt: number
}

export interface PendingQuestion {
  id: string
  text: string
}

export interface QueuedSubmission {
  id: string
  kind: 'prompt'
  text: string
  queuedAt: number
}

export interface AgentStreamState {
  text: string
  thinking: string
}

export interface TeamState {
  name: string
  members: TeamMemberInfo[]
  pendingMessages: number
  recentMessages: Array<{ from: string; to: string; summary: string; timestamp: number }>
}

export interface SubsystemState {
  lsp: LspServerInfo[]
  mcp: McpServerStatusInfo[]
  plugins: PluginInfo[]
  skills: SkillInfo[]
  lastUpdated: number
}

/**
 * Latest diagnostics snapshot per document URI. The Rust backend emits
 * `LspEvent::DiagnosticsPublished { uri, diagnostics }` each time a
 * language server replies with a `textDocument/publishDiagnostics`
 * notification; we keep only the most recent list per URI so the
 * `DiagnosticsDisplay` component can render either the full per-file
 * breakdown (verbose mode) or the aggregate counts (default).
 *
 * The map is cleared when the document is removed from the LSP's
 * tracking (empty `diagnostics` array overwrites the previous entry),
 * so downstream counting remains accurate without extra plumbing.
 */
export interface DiagnosticsState {
  byUri: Record<string, LspDiagnostic[]>
  lastUpdated: number
}

/**
 * Optional selection descriptor shipped by an IDE extension. Mirrors the
 * upstream `IDESelection` payload `ui/examples/upstream-patterns/src/
 * hooks/useIdeSelection.ts` reads. cc-rust's IDE integration currently
 * forwards selection via MCP (`ide` server); we stash the latest value
 * here so `IdeStatusIndicator` does not need to wire to MCP directly.
 *
 * A snapshot with `lineCount === 0` is treated as "file-only" — we render
 * the filename badge instead of the line-range badge.
 */
export interface IdeSelectionSnapshot {
  /** Absolute path of the file containing the selection. */
  filePath?: string
  /** Selected text, when the IDE provides it. */
  text?: string
  /** Number of selected lines (0 when the cursor is idle in the file). */
  lineCount: number
  /** Timestamp the snapshot was last refreshed, for debugging. */
  updatedAt: number
}

/**
 * IDE integration state. `connected` tracks whether any IDE integration
 * (terminal-attached + MCP) is currently live so components can render
 * IDE-aware chrome (e.g. status indicator) without inspecting MCP server
 * lists. `selection` carries the most recent selection snapshot shipped
 * by the IDE extension.
 */
export interface IdeState {
  connected: boolean
  selection: IdeSelectionSnapshot | null
}

/**
 * Transient state for the LSP plugin recommendation prompt.
 * `request` is populated while the dialog is visible; `settings` mirrors
 * the persisted "never/disable" choices so an LSP settings view can
 * render them without re-querying the backend.
 */
export interface LspRecommendationState {
  request: LspRecommendationPayload | null
  settings: LspRecommendationSettings
}

/**
 * Latest `status_line_update` payload forwarded from the backend. When
 * the user has configured a custom statusline command, the Rust side
 * runs it on a cadence and ships the rendered stdout here as `lines`.
 * The built-in statusline is always available as a fallback;
 * `customStatusLine` is additive, not a replacement.
 */
export interface CustomStatusLineState {
  lines: string[]
  error?: string
  updatedAt: number
}

/**
 * State for the `/mcp` management dialog.
 *
 * Mirrors the upstream `MCPViewState` machine (`ui/examples/upstream-patterns/
 * src/components/mcp/types.ts`) adapted to the IPC surface we already have:
 *
 * - `entries` — editable config list (`McpEvent::ConfigList`).
 * - `status` — live connection state (`McpEvent::ServerList` or streamed
 *   `McpEvent::ServerStateChanged`).
 * - `toolsByServer` / `resourcesByServer` — populated by
 *   `McpEvent::ToolsDiscovered` / `ResourcesDiscovered`.
 * - `lastError` / `lastMessage` — inline feedback for the current view.
 * - `open` toggles rendering of the `McpDialog`.
 */
export interface McpSettingsState {
  open: boolean
  entries: McpServerConfigEntry[]
  status: McpServerStatusInfo[]
  toolsByServer: Record<string, McpToolInfo[]>
  resourcesByServer: Record<string, McpResourceInfo[]>
  lastError: string | null
  lastMessage: string | null
  lastUpdated: number
}

/**
 * State for the `/agents` settings dialog. `entries` is the full list
 * returned by the backend's `AgentSettingsEvent::List`; `lastError` surfaces
 * the most recent `error` event so the dialog can show an inline message.
 *
 * `availableTools` caches the last categorized tool-list response — the
 * `ToolSelector` step uses it and only re-queries when stale. `generating`
 * is `true` between `GenerateStarted` and `Generated`/`Error` so the
 * wizard's `GenerateStep` can show a spinner, and `lastGenerated` carries
 * the AI-produced draft back into the wizard's subsequent steps.
 */
export interface AgentSettingsState {
  entries: AgentDefinitionEntry[]
  open: boolean
  lastError: string | null
  lastMessage: string | null
  lastUpdated: number
  availableTools: AgentToolInfo[]
  toolsLoadedAt: number
  generating: boolean
  lastGenerated:
    | { identifier: string; whenToUse: string; systemPrompt: string }
    | null
}

export interface AppState {
  messages: RawMessage[]
  streamingText: string
  streamingThinking: string
  streamingMessageId: string | null
  isStreaming: boolean
  isWaiting: boolean
  model: string
  sessionId: string
  cwd: string
  usage: Usage
  permissionMode: string
  planWorkflow: PlanWorkflowRecord | null
  permissionRequest: PermissionRequest | null
  pendingQuestion: PendingQuestion | null
  suggestions: string[]
  inputHistory: string[]
  historyIndex: number
  editorMode: EditorMode
  vimEnabled: boolean
  vimMode: string
  keybindingConfig: KeybindingConfig | null
  backgroundAgents: BackgroundAgent[]
  shellProgress: Record<string, ShellProgressState>
  queuedSubmissions: QueuedSubmission[]
  viewMode: ViewMode
  agentTree: AgentNode[]
  agentStreams: Record<string, AgentStreamState>
  teams: Record<string, TeamState>
  subsystems: SubsystemState
  diagnostics: DiagnosticsState
  ide: IdeState
  lspRecommendation: LspRecommendationState
  customStatusLine: CustomStatusLineState | null
  agentSettings: AgentSettingsState
  mcpSettings: McpSettingsState
}

export const initialState: AppState = {
  messages: [],
  streamingText: '',
  streamingThinking: '',
  streamingMessageId: null,
  isStreaming: false,
  isWaiting: false,
  model: '',
  sessionId: '',
  cwd: '',
  usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
  permissionMode: 'default',
  planWorkflow: null,
  permissionRequest: null,
  pendingQuestion: null,
  suggestions: [],
  inputHistory: [],
  historyIndex: -1,
  editorMode: 'normal',
  vimEnabled: false,
  vimMode: 'NORMAL',
  keybindingConfig: null,
  backgroundAgents: [],
  shellProgress: {},
  queuedSubmissions: [],
  viewMode: 'prompt',
  agentTree: [],
  agentStreams: {},
  teams: {},
  subsystems: { lsp: [], mcp: [], plugins: [], skills: [], lastUpdated: 0 },
  diagnostics: { byUri: {}, lastUpdated: 0 },
  ide: { connected: false, selection: null },
  lspRecommendation: { request: null, settings: { disabled: false, muted_plugins: [] } },
  customStatusLine: null,
  agentSettings: {
    entries: [],
    open: false,
    lastError: null,
    lastMessage: null,
    lastUpdated: 0,
    availableTools: [],
    toolsLoadedAt: 0,
    generating: false,
    lastGenerated: null,
  },
  mcpSettings: {
    open: false,
    entries: [],
    status: [],
    toolsByServer: {},
    resourcesByServer: {},
    lastError: null,
    lastMessage: null,
    lastUpdated: 0,
  },
}

export type CoreAction =
  | {
      type: 'READY'
      model: string
      sessionId: string
      cwd: string
      permissionMode?: string | null
      planWorkflow?: PlanWorkflowRecord | null
      editorMode?: string | null
      viewMode?: ViewMode | null
      keybindings?: KeybindingConfig | null
    }
  | { type: 'REPLACE_MESSAGES'; messages: RawMessage[] }
  | { type: 'ADD_USER_MESSAGE'; id: string; text: string }
  | { type: 'ADD_COMMAND_MESSAGE'; id: string; text: string }
  | { type: 'STREAM_START'; messageId: string }
  | { type: 'STREAM_DELTA'; text: string }
  | { type: 'THINKING_DELTA'; thinking: string }
  | { type: 'STREAM_END' }
  | {
      type: 'ASSISTANT_MESSAGE'
      id: string
      content: string
      contentBlocks?: FrontendContentBlock[]
      costUsd: number
      thinking?: string
    }
  | { type: 'PERMISSION_REQUEST'; request: PermissionRequest }
  | { type: 'PERMISSION_DISMISS' }
  | { type: 'QUESTION_REQUEST'; question: PendingQuestion }
  | { type: 'QUESTION_DISMISS' }
  | { type: 'PLAN_WORKFLOW_EVENT'; event: string; summary: string; record: PlanWorkflowRecord }
  | { type: 'SYSTEM_INFO'; text: string; level: string }
  | { type: 'USAGE_UPDATE'; usage: Usage }
  | { type: 'SUGGESTIONS'; items: string[] }
  | { type: 'ERROR'; message: string }

export type ToolActivityAction =
  | { type: 'TOOL_USE'; id: string; name: string; input: any }
  | { type: 'TOOL_RESULT'; toolUseId: string; output: string; isError: boolean }
  | {
      type: 'TOOL_PROGRESS'
      toolUseId: string
      tool: string
      output: string
      elapsedSeconds: number
      totalLines?: number
      totalBytes?: number
      timeoutMs?: number
    }

export type BackgroundAgentAction =
  | { type: 'BG_AGENT_STARTED'; agentId: string; description: string }
  | {
      type: 'BG_AGENT_COMPLETE'
      agentId: string
      description: string
      resultPreview: string
      hadError: boolean
      durationMs: number
    }

export type AgentTreeAction =
  | { type: 'AGENT_TREE_SNAPSHOT'; roots: AgentNode[] }
  | {
      type: 'AGENT_SPAWNED'
      agentId: string
      description: string
      parentAgentId?: string
      agentType?: string
      model?: string
      isBackground: boolean
      depth: number
    }
  | { type: 'AGENT_COMPLETED'; agentId: string; resultPreview: string; hadError: boolean; durationMs: number }
  | { type: 'AGENT_ERROR'; agentId: string; error: string; durationMs: number }
  | { type: 'AGENT_ABORTED'; agentId: string }
  | { type: 'AGENT_STREAM_DELTA'; agentId: string; text: string }
  | { type: 'AGENT_THINKING_DELTA'; agentId: string; thinking: string }

export type TeamAction =
  | { type: 'TEAM_MEMBER_JOINED'; teamName: string; agentId: string; agentName: string; role: string }
  | { type: 'TEAM_MEMBER_LEFT'; teamName: string; agentId: string }
  | { type: 'TEAM_MESSAGE_ROUTED'; teamName: string; from: string; to: string; summary: string; timestamp: number }
  | { type: 'TEAM_STATUS_SNAPSHOT'; teamName: string; members: TeamMemberInfo[]; pendingMessages: number }

export type SubsystemAction =
  | { type: 'SUBSYSTEM_STATUS'; lsp: LspServerInfo[]; mcp: McpServerStatusInfo[]; plugins: PluginInfo[]; skills: SkillInfo[] }
  | { type: 'LSP_SERVER_STATE'; languageId: string; state: string; error?: string }
  | { type: 'MCP_SERVER_STATE'; serverName: string; state: string; error?: string }
  | { type: 'PLUGIN_STATUS'; pluginId: string; name: string; status: string; error?: string }
  | { type: 'SKILLS_LOADED'; count: number }
  | { type: 'CUSTOM_STATUS_LINE_UPDATE'; lines: string[]; error?: string; updatedAt: number }
  | { type: 'LSP_RECOMMENDATION_REQUEST'; payload: LspRecommendationPayload }
  | { type: 'LSP_RECOMMENDATION_DISMISS' }
  | { type: 'LSP_RECOMMENDATION_SETTINGS'; settings: LspRecommendationSettings }
  | { type: 'LSP_DIAGNOSTICS_PUBLISHED'; uri: string; diagnostics: LspDiagnostic[] }
  | { type: 'LSP_DIAGNOSTICS_CLEAR' }
  | { type: 'IDE_CONNECTION_CHANGED'; connected: boolean }
  | { type: 'IDE_SELECTION_CHANGED'; selection: IdeSelectionSnapshot | null }

export type McpSettingsAction =
  | { type: 'MCP_SETTINGS_OPEN' }
  | { type: 'MCP_SETTINGS_CLOSE' }
  | { type: 'MCP_SETTINGS_CONFIG_LIST'; entries: McpServerConfigEntry[] }
  | {
      type: 'MCP_SETTINGS_CONFIG_CHANGED'
      serverName: string
      entry?: McpServerConfigEntry
    }
  | { type: 'MCP_SETTINGS_CONFIG_ERROR'; serverName: string; error: string }
  | { type: 'MCP_SETTINGS_SERVER_LIST'; servers: McpServerStatusInfo[] }
  | {
      type: 'MCP_SETTINGS_SERVER_STATE'
      serverName: string
      state: string
      error?: string
    }
  | { type: 'MCP_SETTINGS_TOOLS_DISCOVERED'; serverName: string; tools: McpToolInfo[] }
  | {
      type: 'MCP_SETTINGS_RESOURCES_DISCOVERED'
      serverName: string
      resources: McpResourceInfo[]
    }
  | { type: 'MCP_SETTINGS_CLEAR_NOTICE' }

export type AgentSettingsAction =
  | { type: 'AGENT_SETTINGS_OPEN' }
  | { type: 'AGENT_SETTINGS_CLOSE' }
  | { type: 'AGENT_SETTINGS_LIST'; entries: AgentDefinitionEntry[] }
  | { type: 'AGENT_SETTINGS_CHANGED'; name: string; entry?: AgentDefinitionEntry }
  | { type: 'AGENT_SETTINGS_ERROR'; name: string; error: string }
  | { type: 'AGENT_SETTINGS_CLEAR_NOTICE' }
  | { type: 'AGENT_SETTINGS_TOOLS'; tools: AgentToolInfo[] }
  | { type: 'AGENT_SETTINGS_EDITOR_OPENED'; filePath: string }
  | { type: 'AGENT_SETTINGS_GENERATE_STARTED' }
  | {
      type: 'AGENT_SETTINGS_GENERATED'
      identifier: string
      whenToUse: string
      systemPrompt: string
    }
  | { type: 'AGENT_SETTINGS_CLEAR_GENERATED' }

export type InputAction =
  | { type: 'PUSH_HISTORY'; text: string }
  | { type: 'SET_HISTORY_INDEX'; index: number }
  | { type: 'SET_EDITOR_MODE'; editorMode: EditorMode }
  | { type: 'SET_VIM_MODE'; mode: string }
  | { type: 'SET_KEYBINDINGS_CONFIG'; keybindings: KeybindingConfig | null }
  | { type: 'TOGGLE_VIM' }
  | { type: 'QUEUE_SUBMISSION'; submission: QueuedSubmission }
  | { type: 'DEQUEUE_SUBMISSION' }
  | { type: 'SET_VIEW_MODE'; viewMode: ViewMode }
  | { type: 'TOGGLE_VIEW_MODE' }

export type AppAction =
  | CoreAction
  | ToolActivityAction
  | BackgroundAgentAction
  | AgentTreeAction
  | TeamAction
  | SubsystemAction
  | AgentSettingsAction
  | McpSettingsAction
  | InputAction
