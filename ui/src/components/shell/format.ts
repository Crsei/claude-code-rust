/**
 * Formatting helpers for shell UI — localized here so the shell components
 * don't pull in a utility surface that doesn't exist in the OpenTUI port.
 *
 * - `formatDuration`: same semantics as the upstream `utils/format.ts`
 *   helper — produces `0.4s`, `12s`, `3m 5s`, `1h 2m`, etc.
 * - `formatFileSize`: short byte count like `128B`, `4.2KB`, `1.8MB`.
 * - `stripAnsi`: minimal ANSI escape stripper for terminal output we
 *   want to render as plain text (OpenTUI `<text>` does not interpret
 *   ANSI control sequences).
 */

export interface FormatDurationOptions {
  /** Drop trailing `0s`/`0m` segments from compound outputs. */
  hideTrailingZeros?: boolean
}

export function formatDuration(
  ms: number,
  options: FormatDurationOptions = {},
): string {
  if (!Number.isFinite(ms) || ms < 0) {
    return '0s'
  }

  const { hideTrailingZeros = false } = options

  if (ms < 1000) {
    return `${(ms / 1000).toFixed(1)}s`
  }

  const totalSeconds = Math.floor(ms / 1000)
  if (totalSeconds < 60) {
    return `${totalSeconds}s`
  }

  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes < 60) {
    if (hideTrailingZeros && seconds === 0) {
      return `${minutes}m`
    }
    return `${minutes}m ${seconds}s`
  }

  const hours = Math.floor(minutes / 60)
  const remMinutes = minutes % 60
  if (hideTrailingZeros && remMinutes === 0) {
    return `${hours}h`
  }
  return `${hours}h ${remMinutes}m`
}

export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return '0B'
  }
  if (bytes < 1024) {
    return `${bytes}B`
  }
  const kb = bytes / 1024
  if (kb < 1024) {
    return `${kb < 10 ? kb.toFixed(1) : Math.round(kb)}KB`
  }
  const mb = kb / 1024
  if (mb < 1024) {
    return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)}MB`
  }
  const gb = mb / 1024
  return `${gb < 10 ? gb.toFixed(1) : Math.round(gb)}GB`
}

// eslint-disable-next-line no-control-regex
const ANSI_PATTERN = /\u001b\[[0-9;?]*[A-Za-z]|\u001b\]([^\u0007\u001b]*)(?:\u0007|\u001b\\)/g

/**
 * Strip CSI/OSC ANSI escape sequences from `value` so the result can be
 * rendered by plain OpenTUI `<text>` nodes.
 */
export function stripAnsi(value: string): string {
  return value.replace(ANSI_PATTERN, '')
}
