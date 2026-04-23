import type {
  ConfigScope,
  McpResourceInfo,
  McpServerConfigEntry,
  McpServerStatusInfo,
  McpToolInfo,
} from '../../ipc/protocol.js'

/**
 * Joined view of a configured MCP server — pairs the editable config
 * entry (`McpServerConfigEntry`) with its live status row
 * (`McpServerStatusInfo`) so every sub-view has everything it needs
 * to render without plumbing the lookup through each prop.
 *
 * Adapted from the upstream `ServerInfo` type; the OAuth/claude.ai
 * proxy / session-ingress machinery is intentionally dropped — cc-rust
 * doesn't speak those transports — and replaced with a `disabled`
 * signal that the UI flips via the `toggle_enabled` command.
 */
export interface ServerInfo {
  /** Server name (unique within its scope). */
  name: string
  /** Editable config payload. */
  config: McpServerConfigEntry
  /**
   * Live connection status. Absent until the backend emits a
   * `ServerList` or `ServerStateChanged` for this server — callers
   * render a "pending" placeholder in that case.
   */
  status?: McpServerStatusInfo
  /** Transport kind (stdio / sse / streamable-http). */
  transport: string
  /** Config scope (user, project, plugin, ide). */
  scope: ConfigScope
  /** Cached tool list (from `ToolsDiscovered`), if any. */
  tools: McpToolInfo[]
  /** Cached resource list (from `ResourcesDiscovered`), if any. */
  resources: McpResourceInfo[]
}

/**
 * Navigation states inside the MCP dialog. Matches the upstream state
 * machine but drops the `agent-server-menu` branch (agents in cc-rust
 * don't carry their own MCP server definitions through IPC yet).
 */
export type MCPViewState =
  | { type: 'list'; defaultTab?: string }
  | { type: 'server-menu'; server: ServerInfo }
  | { type: 'server-tools'; server: ServerInfo }
  | { type: 'server-tool-detail'; server: ServerInfo; toolIndex: number }
