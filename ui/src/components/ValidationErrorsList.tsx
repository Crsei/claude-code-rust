import React from 'react'
import { c } from '../theme.js'

/**
 * Lite-native port of the sample tree's `ValidationErrorsList`
 * (`ui/examples/upstream-patterns/src/components/ValidationErrorsList.tsx`).
 *
 * The upstream version pulls in `lodash-es`, a treeify helper, and the
 * full Ink theme. This Lite variant keeps the visual shape — errors
 * grouped by file, paths rendered hierarchically, optional suggestion /
 * documentation link — but avoids those dependencies so it can live in
 * the Lite UI bundle.
 *
 * The grouping helper `groupValidationErrors` is exported so tests can
 * verify the ordering without mounting the component.
 */

export type ValidationError = {
  /** File that triggered the error. `null` / missing → shown as
   * `(file not specified)`. */
  file?: string | null
  /** Dot-notation path into the config (e.g. `mcp.servers.0.name`). */
  path?: string | null
  /** Human-readable error message. */
  message: string
  /** Optional invalid value for the path — shown verbatim when present. */
  invalidValue?: unknown
  /** Optional remediation hint shown below the grouped path list. */
  suggestion?: string
  /** Optional documentation link shown below the grouped path list. */
  docLink?: string
}

export type GroupedValidationErrors = {
  file: string
  errors: ValidationError[]
  suggestionPairs: Array<{ suggestion?: string; docLink?: string }>
}

const UNSPECIFIED_FILE = '(file not specified)'

function formatInvalidValue(value: unknown): string {
  if (value === null) return 'null'
  if (value === undefined) return 'undefined'
  if (typeof value === 'string') return `"${value}"`
  return String(value)
}

function sortErrorsByPath(errors: ValidationError[]): ValidationError[] {
  return [...errors].sort((a, b) => {
    const ap = a.path ?? ''
    const bp = b.path ?? ''
    if (!ap && bp) return -1
    if (ap && !bp) return 1
    return ap.localeCompare(bp)
  })
}

/**
 * Groups errors by file, sorts files alphabetically, and deduplicates
 * suggestion / docLink pairs within each group.
 */
export function groupValidationErrors(
  errors: ValidationError[],
): GroupedValidationErrors[] {
  const byFile = new Map<string, ValidationError[]>()
  for (const err of errors) {
    const key = err.file ?? UNSPECIFIED_FILE
    const list = byFile.get(key)
    if (list) {
      list.push(err)
    } else {
      byFile.set(key, [err])
    }
  }

  const groups: GroupedValidationErrors[] = []
  const sortedFiles = [...byFile.keys()].sort()
  for (const file of sortedFiles) {
    const fileErrors = sortErrorsByPath(byFile.get(file) ?? [])
    const suggestions = new Map<string, { suggestion?: string; docLink?: string }>()
    for (const err of fileErrors) {
      if (!err.suggestion && !err.docLink) continue
      const key = `${err.suggestion ?? ''}|${err.docLink ?? ''}`
      if (!suggestions.has(key)) {
        suggestions.set(key, { suggestion: err.suggestion, docLink: err.docLink })
      }
    }
    groups.push({
      file,
      errors: fileErrors,
      suggestionPairs: [...suggestions.values()],
    })
  }
  return groups
}

type PathRowProps = {
  error: ValidationError
}

function PathRow({ error }: PathRowProps) {
  const rawPath = error.path ?? ''
  const value =
    error.invalidValue !== undefined
      ? ` = ${formatInvalidValue(error.invalidValue)}`
      : ''
  const prefix = rawPath ? `${rawPath}${value}: ` : ''
  return (
    <text fg={c.dim} selectable>
      {prefix}
      <span fg={c.error}>{error.message}</span>
    </text>
  )
}

type Props = {
  errors: ValidationError[]
}

export function ValidationErrorsList({ errors }: Props) {
  if (errors.length === 0) return null

  const groups = groupValidationErrors(errors)

  return (
    <box flexDirection="column">
      {groups.map(group => (
        <box key={group.file} flexDirection="column" marginBottom={1}>
          <text>
            <strong>{group.file}</strong>
          </text>
          <box flexDirection="column" marginLeft={1}>
            {group.errors.map((err, i) => (
              <PathRow key={`${err.path ?? ''}-${i}`} error={err} />
            ))}
          </box>
          {group.suggestionPairs.length > 0 && (
            <box flexDirection="column" marginTop={1}>
              {group.suggestionPairs.map((pair, i) => (
                <box key={i} flexDirection="column" marginBottom={1}>
                  {pair.suggestion && (
                    <text fg={c.dim}>{pair.suggestion}</text>
                  )}
                  {pair.docLink && (
                    <text fg={c.dim}>Learn more: {pair.docLink}</text>
                  )}
                </box>
              ))}
            </box>
          )}
        </box>
      ))}
    </box>
  )
}
