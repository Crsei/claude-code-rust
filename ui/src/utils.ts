/** Generate a simple unique ID */
export function uid(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
}

/** Format a cost value for display */
export function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(6)}`
  if (usd < 1) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(2)}`
}

/** Format token count with K/M suffixes */
export function formatTokens(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`
  return count.toString()
}

/** Truncate text with ellipsis */
export function truncate(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text
  return text.slice(0, maxLen - 1) + '\u2026'
}
