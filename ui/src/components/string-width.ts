/**
 * Minimal terminal-width estimate for a string.
 *
 * Counts each code point as 1 column, with an extra column added for the
 * common East Asian Wide / Fullwidth ranges. Good enough for layout math in
 * the Lite frontend (tab labels, compact selectors) without pulling in the
 * full `string-width` dependency used by the sample tree.
 *
 * The sample tree uses `stringWidth` from `@anthropic/ink` which delegates
 * to `wcwidth`. This helper is intentionally simpler: it does not attempt
 * to track zero-width joiners, variation selectors, or combining marks.
 * If you need more accuracy, switch to the upstream library later.
 */
export function stringWidth(str: string): number {
  let width = 0
  for (const ch of str) {
    const cp = ch.codePointAt(0)
    if (cp === undefined) continue
    // Skip control characters (C0 + DEL + C1) — they contribute no columns.
    if (cp < 0x20 || (cp >= 0x7f && cp <= 0x9f)) continue
    if (isWide(cp)) {
      width += 2
    } else {
      width += 1
    }
  }
  return width
}

function isWide(cp: number): boolean {
  return (
    (cp >= 0x1100 && cp <= 0x115f) || // Hangul Jamo
    (cp >= 0x2e80 && cp <= 0x303e) || // CJK Radicals, Kangxi
    (cp >= 0x3041 && cp <= 0x33ff) || // Hiragana/Katakana/CJK symbols
    (cp >= 0x3400 && cp <= 0x4dbf) || // CJK Extension A
    (cp >= 0x4e00 && cp <= 0x9fff) || // CJK Unified Ideographs
    (cp >= 0xa000 && cp <= 0xa4cf) || // Yi Syllables
    (cp >= 0xac00 && cp <= 0xd7a3) || // Hangul Syllables
    (cp >= 0xf900 && cp <= 0xfaff) || // CJK Compatibility Ideographs
    (cp >= 0xfe30 && cp <= 0xfe4f) || // CJK Compatibility Forms
    (cp >= 0xff00 && cp <= 0xff60) || // Fullwidth forms
    (cp >= 0xffe0 && cp <= 0xffe6) || // Fullwidth signs
    (cp >= 0x20000 && cp <= 0x2fffd) || // CJK Extension B..F
    (cp >= 0x30000 && cp <= 0x3fffd) // CJK Extension G..
  )
}

/**
 * Truncate `text` so that its display width does not exceed `maxWidth`.
 * Returns the original string when it already fits. When truncation is
 * needed, appends the ellipsis `…` (which has width 1) inside the budget.
 */
export function truncateToWidth(text: string, maxWidth: number): string {
  if (maxWidth <= 0) return ''
  if (stringWidth(text) <= maxWidth) return text
  const ellipsis = '\u2026'
  const budget = Math.max(0, maxWidth - stringWidth(ellipsis))
  let width = 0
  let out = ''
  for (const ch of text) {
    const w = stringWidth(ch)
    if (width + w > budget) break
    out += ch
    width += w
  }
  return out + ellipsis
}
