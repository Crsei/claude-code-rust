/** Move to previous character position */
export function prevCharPos(input: string, pos: number): number {
  if (pos <= 0) return 0
  // Handle surrogate pairs
  const code = input.charCodeAt(pos - 1)
  if (code >= 0xDC00 && code <= 0xDFFF && pos >= 2) {
    return pos - 2
  }
  return pos - 1
}

/** Move to next character position */
export function nextCharPos(input: string, pos: number): number {
  if (pos >= input.length) return input.length
  const code = input.charCodeAt(pos)
  if (code >= 0xD800 && code <= 0xDBFF && pos + 1 < input.length) {
    return pos + 2
  }
  return pos + 1
}

/** Move forward to start of next word (w motion) */
export function wordEndPos(input: string, pos: number): number {
  const len = input.length
  if (pos >= len) return len
  let p = pos

  // Skip current word characters
  while (p < len && input[p] !== ' ' && input[p] !== '\t') p++
  // Skip whitespace
  while (p < len && (input[p] === ' ' || input[p] === '\t')) p++

  return p
}

/** Move backward to start of current/previous word (b motion) */
export function wordStartPos(input: string, pos: number): number {
  if (pos <= 0) return 0
  let p = pos

  // Skip whitespace backwards
  while (p > 0 && (input[p - 1] === ' ' || input[p - 1] === '\t')) p--
  // Skip word characters backwards
  while (p > 0 && input[p - 1] !== ' ' && input[p - 1] !== '\t') p--

  return p
}

/** Move to end of current word inclusive (e motion) */
export function wordEndInclusivePos(input: string, pos: number): number {
  const len = input.length
  if (pos >= len) return len
  let p = pos

  if (input[p] === ' ' || input[p] === '\t') {
    // On whitespace, skip to next word
    while (p < len && (input[p] === ' ' || input[p] === '\t')) p++
  } else {
    p++
  }

  // Move to end of word
  while (p < len && input[p] !== ' ' && input[p] !== '\t') p++

  // Back up one to be inclusive
  if (p > pos + 1) p--

  return p
}

/** Find first non-whitespace character (^ motion) */
export function firstNonWhitespace(input: string): number {
  for (let i = 0; i < input.length; i++) {
    if (input[i] !== ' ' && input[i] !== '\t') return i
  }
  return 0
}
