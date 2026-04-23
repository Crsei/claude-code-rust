/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TrustDialog/utils.ts`.
 *
 * The upstream helpers read settings JSON off disk via `getSettingsForSource`
 * and the permission loader — both live on the TypeScript backend in the
 * Rust port. These functions accept the already-loaded settings snapshots
 * via injected callbacks so the dialog can stay a pure component.
 *
 * `formatListWithAnd` matches upstream character-for-character and is safe
 * to reuse.
 */

export type SettingsSnapshot = {
  path: string
  settings: {
    disableAllHooks?: boolean
    statusLine?: unknown
    fileSuggestion?: unknown
    hooks?: Record<string, unknown[]>
    otelHeadersHelper?: unknown
    apiKeyHelper?: unknown
    awsAuthRefresh?: unknown
    awsCredentialExport?: unknown
    gcpAuthRefresh?: unknown
    env?: Record<string, unknown>
  } | null
}

export type BashRule = {
  ruleBehavior: 'allow' | 'deny'
  toolName: string
}

export type PermissionSnapshot = {
  path: string
  rules: BashRule[]
}

function hasHooks(settings: SettingsSnapshot['settings']): boolean {
  if (settings === null || settings.disableAllHooks) return false
  if (settings.statusLine) return true
  if (settings.fileSuggestion) return true
  if (!settings.hooks) return false
  for (const hookConfig of Object.values(settings.hooks)) {
    if (Array.isArray(hookConfig) && hookConfig.length > 0) return true
  }
  return false
}

export function getHooksSources(settingSources: SettingsSnapshot[]): string[] {
  return settingSources
    .filter(s => hasHooks(s.settings))
    .map(s => s.path)
}

function hasBashPermission(rules: BashRule[]): boolean {
  return rules.some(
    rule =>
      rule.ruleBehavior === 'allow' &&
      (rule.toolName === 'Bash' || rule.toolName.startsWith('Bash(')),
  )
}

export function getBashPermissionSources(
  permissionSources: PermissionSnapshot[],
): string[] {
  return permissionSources
    .filter(p => hasBashPermission(p.rules))
    .map(p => p.path)
}

export function formatListWithAnd(items: string[], limit?: number): string {
  if (items.length === 0) return ''
  const effectiveLimit = limit === 0 ? undefined : limit
  if (!effectiveLimit || items.length <= effectiveLimit) {
    if (items.length === 1) return items[0]!
    if (items.length === 2) return `${items[0]} and ${items[1]}`
    const lastItem = items[items.length - 1]!
    const allButLast = items.slice(0, -1)
    return `${allButLast.join(', ')}, and ${lastItem}`
  }
  const shown = items.slice(0, effectiveLimit)
  const remaining = items.length - effectiveLimit
  if (shown.length === 1) {
    return `${shown[0]} and ${remaining} more`
  }
  return `${shown.join(', ')}, and ${remaining} more`
}

function hasOtelHeadersHelper(s: SettingsSnapshot['settings']): boolean {
  return !!s?.otelHeadersHelper
}

export function getOtelHeadersHelperSources(
  sources: SettingsSnapshot[],
): string[] {
  return sources.filter(s => hasOtelHeadersHelper(s.settings)).map(s => s.path)
}

function hasApiKeyHelper(s: SettingsSnapshot['settings']): boolean {
  return !!s?.apiKeyHelper
}

export function getApiKeyHelperSources(
  sources: SettingsSnapshot[],
): string[] {
  return sources.filter(s => hasApiKeyHelper(s.settings)).map(s => s.path)
}

function hasAwsCommands(s: SettingsSnapshot['settings']): boolean {
  return !!(s?.awsAuthRefresh || s?.awsCredentialExport)
}

export function getAwsCommandsSources(
  sources: SettingsSnapshot[],
): string[] {
  return sources.filter(s => hasAwsCommands(s.settings)).map(s => s.path)
}

function hasGcpCommands(s: SettingsSnapshot['settings']): boolean {
  return !!s?.gcpAuthRefresh
}

export function getGcpCommandsSources(
  sources: SettingsSnapshot[],
): string[] {
  return sources.filter(s => hasGcpCommands(s.settings)).map(s => s.path)
}

/**
 * Upstream uses `SAFE_ENV_VARS` from `utils/managedEnvConstants.ts`; the
 * Rust port doesn't currently ship that set. Callers can pass their own
 * safe-list in through `safeEnvVars`, otherwise the function treats any
 * non-empty `env` record as "has dangerous env vars".
 */
function hasDangerousEnvVars(
  s: SettingsSnapshot['settings'],
  safeEnvVars: ReadonlySet<string>,
): boolean {
  if (!s?.env) return false
  return Object.keys(s.env).some(key => !safeEnvVars.has(key.toUpperCase()))
}

export function getDangerousEnvVarsSources(
  sources: SettingsSnapshot[],
  safeEnvVars: ReadonlySet<string> = new Set(),
): string[] {
  return sources
    .filter(s => hasDangerousEnvVars(s.settings, safeEnvVars))
    .map(s => s.path)
}
