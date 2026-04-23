import type {
  ConfigScope,
  McpServerConfigEntry,
  McpServerStatusInfo,
  McpToolInfo,
} from '../../ipc/protocol.js'
import type { ServerInfo } from './types.js'

/**
 * Port of upstream `services/mcp/utils` helpers, restricted to the
 * shapes cc-rust already ships over IPC. OAuth / session-ingress /
 * elicitation helpers are deliberately omitted — see `types.ts` for
 * the scope boundary.
 */

/** Short human-readable label for a scope. */
export function getScopeLabel(scope: ConfigScope): string {
  switch (scope.kind) {
    case 'user':
      return 'user'
    case 'project':
      return 'project'
    case 'plugin':
      return `plugin${scope.id ? `:${scope.id}` : ''}`
    case 'ide':
      return `ide${scope.id ? `:${scope.id}` : ''}`
  }
}

/** Heading text + tooltip-ish path for a scope section in the list panel. */
export function describeMcpConfigFilePath(scope: ConfigScope): string {
  switch (scope.kind) {
    case 'user':
      return '~/.cc-rust/settings.json'
    case 'project':
      return '.cc-rust/settings.json'
    case 'plugin':
      return scope.id ? `plugin:${scope.id}` : 'plugin-contributed'
    case 'ide':
      return scope.id ? `ide:${scope.id}` : 'ide-contributed'
  }
}

/** True if the scope is writable via the `upsert_config` command. */
export function isEditableScope(scope: ConfigScope): boolean {
  return scope.kind === 'user' || scope.kind === 'project'
}

/** Return the subset of tools that belong to `serverName`. */
export function filterToolsByServer(
  tools: McpToolInfo[] | undefined,
  _serverName: string,
): McpToolInfo[] {
  // `McpEvent::tools_discovered` scopes tools per server already, so
  // the caller has a per-server list. This helper exists for symmetry
  // with upstream callers that pass a flat array; when we get a scoped
  // list (always true today) it's the identity.
  return tools ?? []
}

/**
 * Merge the latest config list + status list + per-server tool/resource
 * caches into the `ServerInfo` rows each view expects.
 *
 * Duplicate names across scopes (e.g. user + project overrides) are
 * preserved — scope grouping in `MCPListPanel` separates them visually.
 */
export interface BuildServerInfosArgs {
  entries: McpServerConfigEntry[]
  status: McpServerStatusInfo[]
  toolsByServer: Record<string, McpToolInfo[]>
  resourcesByServer: Record<string, import('../../ipc/protocol.js').McpResourceInfo[]>
}

export function buildServerInfos(args: BuildServerInfosArgs): ServerInfo[] {
  const statusByName = new Map<string, McpServerStatusInfo>()
  for (const s of args.status) {
    statusByName.set(s.name, s)
  }
  return args.entries.map(entry => {
    const status = statusByName.get(entry.name)
    return {
      name: entry.name,
      config: entry,
      status,
      transport: entry.transport,
      scope: entry.scope,
      tools: args.toolsByServer[entry.name] ?? [],
      resources: args.resourcesByServer[entry.name] ?? [],
    }
  })
}

/**
 * Display-state label derived from `disabled` + live status.
 *
 * Priority order matches the upstream renderer: `disabled` wins over
 * any live state so toggling "Disable" is reflected immediately even
 * while a connection attempt is still in flight.
 */
export function serverDisplayState(
  info: ServerInfo,
): 'disabled' | 'connected' | 'pending' | 'failed' | 'unknown' {
  if (info.config.disabled) return 'disabled'
  const raw = info.status?.state
  if (!raw) return 'unknown'
  if (raw === 'connected') return 'connected'
  if (raw === 'pending' || raw === 'connecting') return 'pending'
  if (raw === 'error' || raw === 'failed') return 'failed'
  if (raw === 'disabled') return 'disabled'
  return 'unknown'
}

/** Title-case the first character. Used for dialog headers. */
export function capitalize(s: string): string {
  if (!s) return s
  return s.charAt(0).toUpperCase() + s.slice(1)
}

/** Convenience: pluralise a count (matches upstream `plural()`). */
export function plural(count: number, word: string): string {
  return count === 1 ? word : `${word}s`
}
