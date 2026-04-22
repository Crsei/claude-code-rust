/**
 * Shared state-to-colour mapping for operational panels (MCP / LSP /
 * plugins / teams). Lifted from the original `SubsystemStatus.tsx`
 * module so every subsystem card reads from the same palette.
 *
 * Keep the mapping keyed on the raw protocol strings (`'running'`,
 * `'connected'`, `'installed'`, etc.) — the Rust backend forwards
 * these verbatim, and every card takes the state as-is.
 */

const STATE_COLORS: Record<string, string> = {
  // Green family — healthy
  running: '#A6E3A1',
  connected: '#A6E3A1',
  installed: '#A6E3A1',
  enabled: '#A6E3A1',
  ready: '#A6E3A1',
  // Amber family — in-flight
  starting: '#F9E2AF',
  connecting: '#F9E2AF',
  installing: '#F9E2AF',
  reconnecting: '#F9E2AF',
  // Grey family — dormant / explicit opt-out
  stopped: '#6C7086',
  disconnected: '#6C7086',
  disabled: '#6C7086',
  not_installed: '#6C7086',
  idle: '#6C7086',
  // Red family — failures
  error: '#F38BA8',
  failed: '#F38BA8',
  crashed: '#F38BA8',
}

const DEFAULT_COLOR = '#6C7086'

export function stateColor(state: string | undefined | null): string {
  if (!state) return DEFAULT_COLOR
  return STATE_COLORS[state.toLowerCase()] ?? DEFAULT_COLOR
}

export function isHealthyState(state: string | undefined | null): boolean {
  if (!state) return false
  const lowered = state.toLowerCase()
  return (
    lowered === 'running' ||
    lowered === 'connected' ||
    lowered === 'installed' ||
    lowered === 'enabled' ||
    lowered === 'ready'
  )
}
