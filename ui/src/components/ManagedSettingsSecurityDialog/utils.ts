/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/ManagedSettingsSecurityDialog/utils.ts`.
 *
 * Upstream reads the authoritative `DANGEROUS_SHELL_SETTINGS` and
 * `SAFE_ENV_VARS` lists from `utils/managedEnvConstants.js`. cc-rust
 * does not yet pull managed-settings state through to the frontend, so
 * we mirror the same lists locally. Update them in lockstep whenever
 * the upstream set changes — the values here drive what the dialog
 * flags as "requires approval".
 */

/** Settings fields that can execute arbitrary code or intercept
 *  transcripts — matches upstream `DANGEROUS_SHELL_SETTINGS`. */
export const DANGEROUS_SHELL_SETTINGS = [
  'apiKeyHelper',
  'awsAuthRefresh',
  'awsCredentialExport',
  'shellIntegration',
  'statusLine',
] as const

export type DangerousShellSetting = (typeof DANGEROUS_SHELL_SETTINGS)[number]

/** Env vars considered safe to be set by managed settings. Any `env.*`
 *  entry NOT in this set counts as dangerous. Matches upstream
 *  `SAFE_ENV_VARS` (all keys upper-cased). */
export const SAFE_ENV_VARS: ReadonlySet<string> = new Set([
  'NO_COLOR',
  'FORCE_COLOR',
  'ANTHROPIC_LOG',
  'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC',
  'CLAUDE_CODE_DISABLE_TELEMETRY',
  'CLAUDE_CODE_ENABLE_TELEMETRY',
  'CLAUDE_CODE_USE_BEDROCK',
  'CLAUDE_CODE_USE_VERTEX',
  'DISABLE_TELEMETRY',
  'HTTP_PROXY',
  'HTTPS_PROXY',
  'NO_PROXY',
  'AWS_REGION',
])

/**
 * Shape the dialog consumes. Keep structurally identical to the
 * upstream `DangerousSettings` type so consumers don't need to switch
 * on the Lite variant.
 */
export interface DangerousSettings {
  shellSettings: Partial<Record<DangerousShellSetting, string>>
  envVars: Record<string, string>
  hasHooks: boolean
  hooks?: unknown
}

/**
 * Minimal settings shape the extractor reads. We keep it open (indexed
 * signature) so callers can pass the raw managed-settings JSON without
 * needing to import a full `SettingsJson` definition.
 */
export type SettingsLike = {
  env?: Record<string, unknown> | null
  hooks?: unknown
  [key: string]: unknown
}

export function extractDangerousSettings(
  settings: SettingsLike | null | undefined,
): DangerousSettings {
  if (!settings) {
    return { shellSettings: {}, envVars: {}, hasHooks: false }
  }

  const shellSettings: Partial<Record<DangerousShellSetting, string>> = {}
  for (const key of DANGEROUS_SHELL_SETTINGS) {
    const value = settings[key]
    if (typeof value === 'string' && value.length > 0) {
      shellSettings[key] = value
    }
  }

  const envVars: Record<string, string> = {}
  if (settings.env && typeof settings.env === 'object') {
    for (const [key, value] of Object.entries(settings.env)) {
      if (typeof value === 'string' && value.length > 0) {
        if (!SAFE_ENV_VARS.has(key.toUpperCase())) {
          envVars[key] = value
        }
      }
    }
  }

  const hasHooks =
    settings.hooks !== undefined &&
    settings.hooks !== null &&
    typeof settings.hooks === 'object' &&
    Object.keys(settings.hooks as Record<string, unknown>).length > 0

  return {
    shellSettings,
    envVars,
    hasHooks,
    hooks: hasHooks ? settings.hooks : undefined,
  }
}

export function hasDangerousSettings(dangerous: DangerousSettings): boolean {
  return (
    Object.keys(dangerous.shellSettings).length > 0 ||
    Object.keys(dangerous.envVars).length > 0 ||
    dangerous.hasHooks
  )
}

/**
 * Compare old and new settings and return `true` when the dangerous
 * slice has changed or gained entries. Drives whether the dialog
 * should appear on reload.
 */
export function hasDangerousSettingsChanged(
  oldSettings: SettingsLike | null | undefined,
  newSettings: SettingsLike | null | undefined,
): boolean {
  const oldDangerous = extractDangerousSettings(oldSettings)
  const newDangerous = extractDangerousSettings(newSettings)

  if (!hasDangerousSettings(newDangerous)) return false
  if (!hasDangerousSettings(oldDangerous)) return true

  // Stable stringification — object keys sorted so we don't trip on
  // insertion order.
  const serialize = (d: DangerousSettings): string =>
    JSON.stringify({
      shellSettings: sortKeys(d.shellSettings as Record<string, unknown>),
      envVars: sortKeys(d.envVars),
      hooks: d.hooks,
    })

  return serialize(oldDangerous) !== serialize(newDangerous)
}

function sortKeys(obj: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const key of Object.keys(obj).sort()) {
    out[key] = obj[key]
  }
  return out
}

/** Flatten dangerous settings into a human-readable list of field
 *  names. Values are intentionally omitted — the dialog only needs to
 *  reveal what fields are present. */
export function formatDangerousSettingsList(
  dangerous: DangerousSettings,
): string[] {
  const items: string[] = []
  for (const key of Object.keys(dangerous.shellSettings)) items.push(key)
  for (const key of Object.keys(dangerous.envVars)) items.push(key)
  if (dangerous.hasHooks) items.push('hooks')
  return items
}
