import { truncate } from '../utils.js'

/**
 * Produce a readable one-line rendering of a tool's JSON input. Mirrors
 * `describeToolInput` from `ui/src/store/message-model.ts` but is exposed
 * via the adapter layer so migration slices can reuse the same heuristic
 * without pulling in the whole message-model file.
 */
export function describeToolInput(input: unknown): string {
  if (typeof input === 'string') {
    return normalizeInline(input)
  }
  if (!input || typeof input !== 'object') {
    return normalizeInline(String(input ?? ''))
  }

  const record = input as Record<string, unknown>

  if (typeof record.command === 'string' && record.command.trim()) {
    return normalizeInline(record.command)
  }
  if (typeof record.file_path === 'string' && record.file_path.trim()) {
    return normalizeInline(record.file_path)
  }
  if (typeof record.url === 'string' && record.url.trim()) {
    return normalizeInline(record.url)
  }
  if (typeof record.pattern === 'string' && record.pattern.trim()) {
    const path =
      typeof record.path === 'string' && record.path.trim()
        ? ` in ${record.path}`
        : ''
    return normalizeInline(`"${record.pattern}"${path}`)
  }
  if (typeof record.path === 'string' && record.path.trim()) {
    return normalizeInline(record.path)
  }
  if (typeof record.prompt === 'string' && record.prompt.trim()) {
    return normalizeInline(record.prompt)
  }
  if (typeof record.question === 'string' && record.question.trim()) {
    return normalizeInline(record.question)
  }

  try {
    return normalizeInline(JSON.stringify(input))
  } catch {
    return '(structured input)'
  }
}

export function summarizeToolInput(input: unknown, maxLength = 120): string {
  return truncate(describeToolInput(input), maxLength)
}

function normalizeInline(value: string): string {
  return value.replace(/\s+/g, ' ').trim()
}
