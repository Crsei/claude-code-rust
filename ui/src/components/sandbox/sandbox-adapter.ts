import { createContext, useContext } from 'react'

/**
 * Lite-native replacement for upstream's
 * `utils/sandbox/sandbox-adapter.js`.
 *
 * Upstream keeps a singleton `SandboxManager` that reads/writes the
 * settings file + platform binaries directly. cc-rust's frontend has
 * no FS access, so the sandbox UI is driven by a data-only snapshot
 * the backend ships over IPC (or a local stub used by the config
 * preview). Hosts provide the snapshot through `SandboxAdapterContext`;
 * the individual tabs consume `useSandboxAdapter()` to read + mutate
 * via the supplied callbacks.
 */

export type Platform = 'macos' | 'linux' | 'wsl' | 'windows' | 'unknown'

/** Dependency check result. Mirrors upstream
 *  `SandboxDependencyCheck`. */
export interface SandboxDependencyCheck {
  errors: string[]
  warnings: string[]
}

export interface SandboxFsReadConfig {
  denyOnly: string[]
  /** Paths that escape `denyOnly` — matches upstream
   *  `filesystem.read.allowWithinDeny`. */
  allowWithinDeny?: string[]
}

export interface SandboxFsWriteConfig {
  allowOnly: string[]
  /** Paths that, despite being under `allowOnly`, remain denied. */
  denyWithinAllow: string[]
}

export interface SandboxNetworkConfig {
  allowedHosts?: string[]
  deniedHosts?: string[]
  /** Upstream flag surfaced in the config tab. */
  allowAllUnixSockets?: boolean
}

export interface SandboxSettings {
  enabled: boolean
  autoAllowBashIfSandboxed: boolean
  allowUnsandboxedCommands: boolean
  /** `true` when the local settings are pinned by a managed / policy
   *  source. UI renders a read-only view in that case. */
  lockedByPolicy: boolean
  /** Platform string used by `SandboxDependenciesTab` to choose the
   *  install-hint wording. */
  platform: Platform
  /** Running on a platform where sandboxing can work at all. */
  supportedPlatform: boolean
  /** `true` when settings have sandbox enabled — even when deps are
   *  missing. */
  enabledInSettings: boolean
  excludedCommands: string[]
  fsRead: SandboxFsReadConfig
  fsWrite: SandboxFsWriteConfig
  network: SandboxNetworkConfig
  allowUnixSockets: string[]
  allowManagedSandboxDomainsOnly: boolean
  /** Glob patterns the backend detected it can't enforce on Linux. */
  linuxGlobPatternWarnings: string[]
  dependencyCheck: SandboxDependencyCheck
}

export type SandboxSettingsPatch = Partial<
  Pick<
    SandboxSettings,
    'enabled' | 'autoAllowBashIfSandboxed' | 'allowUnsandboxedCommands'
  >
>

export interface SandboxAdapter {
  settings: SandboxSettings
  /** Persist one or more fields. Adapters forward to the backend's
   *  `/sandbox` command or the Rust side's write-settings IPC. */
  updateSettings: (patch: SandboxSettingsPatch) => Promise<void> | void
}

const DEFAULT_PLATFORM: Platform =
  (globalThis as unknown as { process?: { platform?: string } }).process?.platform === 'darwin'
    ? 'macos'
    : (globalThis as unknown as { process?: { platform?: string } }).process?.platform === 'linux'
      ? 'linux'
      : (globalThis as unknown as { process?: { platform?: string } }).process?.platform === 'win32'
        ? 'windows'
        : 'unknown'

/** Sensible empty snapshot — used by `useSandboxAdapter()` when no
 *  provider is mounted so tabs can still render "Sandbox is not
 *  enabled" without throwing. */
export const EMPTY_SANDBOX_SETTINGS: SandboxSettings = {
  enabled: false,
  autoAllowBashIfSandboxed: false,
  allowUnsandboxedCommands: false,
  lockedByPolicy: false,
  platform: DEFAULT_PLATFORM,
  supportedPlatform: DEFAULT_PLATFORM === 'macos' || DEFAULT_PLATFORM === 'linux',
  enabledInSettings: false,
  excludedCommands: [],
  fsRead: { denyOnly: [] },
  fsWrite: { allowOnly: [], denyWithinAllow: [] },
  network: {},
  allowUnixSockets: [],
  allowManagedSandboxDomainsOnly: false,
  linuxGlobPatternWarnings: [],
  dependencyCheck: { errors: [], warnings: [] },
}

const DEFAULT_ADAPTER: SandboxAdapter = {
  settings: EMPTY_SANDBOX_SETTINGS,
  updateSettings: () => {},
}

export const SandboxAdapterContext = createContext<SandboxAdapter>(DEFAULT_ADAPTER)

export function useSandboxAdapter(): SandboxAdapter {
  return useContext(SandboxAdapterContext)
}

/** Convenience — does the current settings snapshot indicate the
 *  sandbox is fully wired up? Mirrors upstream
 *  `SandboxManager.isSandboxingEnabled()`. */
export function isSandboxingEnabled(settings: SandboxSettings): boolean {
  return (
    settings.enabled &&
    settings.supportedPlatform &&
    settings.dependencyCheck.errors.length === 0
  )
}

/** Pure helper for the config tab — returns `true` when the network
 *  block should be annotated "(Managed)". Mirrors upstream
 *  `shouldAllowManagedSandboxDomainsOnly()`. */
export function shouldAllowManagedSandboxDomainsOnly(
  settings: SandboxSettings,
): boolean {
  return settings.allowManagedSandboxDomainsOnly
}
