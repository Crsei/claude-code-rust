/**
 * Extract file-edit context from a raw tool input shape.
 *
 * The current Rust protocol keeps tool inputs as opaque JSON on the
 * wire, but the two tools we care about here (`Edit` / `MultiEdit` and
 * `Write`) share a small keyed shape: `file_path`, plus either a
 * single `old_string` / `new_string` pair or an `edits` array. This
 * adapter turns that into a normalized `FileEditContext` that the
 * permission dialog and the richer tool-result renderer both consume.
 *
 * Returns `null` when the input doesn't look like a file edit — callers
 * fall back to the generic rendering in that case.
 */

export interface FileEditEntry {
  oldString: string
  newString: string
  replaceAll?: boolean
}

export interface FileEditContext {
  kind: 'edit' | 'multi_edit' | 'write'
  filePath: string
  edits: FileEditEntry[]
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function coerceEditEntry(raw: unknown): FileEditEntry | null {
  if (!raw || typeof raw !== 'object') return null
  const record = raw as Record<string, unknown>
  const oldString = record.old_string ?? record.oldString
  const newString = record.new_string ?? record.newString
  if (typeof oldString !== 'string' || typeof newString !== 'string') {
    return null
  }
  const replaceAllValue = record.replace_all ?? record.replaceAll
  const entry: FileEditEntry = { oldString, newString }
  if (typeof replaceAllValue === 'boolean') {
    entry.replaceAll = replaceAllValue
  }
  return entry
}

export function extractFileEditContext(
  toolName: string,
  input: unknown,
): FileEditContext | null {
  if (!input || typeof input !== 'object') return null
  const record = input as Record<string, unknown>
  const filePath = record.file_path ?? record.filePath ?? record.path
  if (!isNonEmptyString(filePath)) return null

  const nameLower = toolName.toLowerCase()

  if (Array.isArray(record.edits)) {
    const edits = record.edits
      .map(coerceEditEntry)
      .filter((entry): entry is FileEditEntry => entry !== null)
    if (edits.length === 0) return null
    return { kind: 'multi_edit', filePath, edits }
  }

  const single = coerceEditEntry(record)
  if (single) {
    const kind: FileEditContext['kind'] =
      nameLower === 'write' || nameLower === 'filewrite' ? 'write' : 'edit'
    return { kind, filePath, edits: [single] }
  }

  if (nameLower === 'write' || nameLower === 'filewrite') {
    const content = record.content ?? record.text
    if (typeof content === 'string') {
      return {
        kind: 'write',
        filePath,
        edits: [{ oldString: '', newString: content }],
      }
    }
  }

  return null
}
