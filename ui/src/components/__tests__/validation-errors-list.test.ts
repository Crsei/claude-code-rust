import { describe, expect, test } from 'bun:test'
import { groupValidationErrors, type ValidationError } from '../ValidationErrorsList.js'

describe('groupValidationErrors', () => {
  test('groups errors by file and sorts the group keys alphabetically', () => {
    const errors: ValidationError[] = [
      { file: 'b.json', path: 'x', message: 'nope' },
      { file: 'a.json', path: 'y', message: 'bad' },
      { file: 'a.json', path: 'x', message: 'wrong' },
    ]
    const groups = groupValidationErrors(errors)
    expect(groups.map(g => g.file)).toEqual(['a.json', 'b.json'])
    const first = groups[0]!
    expect(first.errors.map(e => e.path)).toEqual(['x', 'y'])
  })

  test('falls back to the unspecified-file label when file is missing', () => {
    const errors: ValidationError[] = [
      { path: 'foo', message: 'oops' },
      { file: null, path: 'bar', message: 'nope' },
    ]
    const groups = groupValidationErrors(errors)
    expect(groups).toHaveLength(1)
    expect(groups[0]!.file).toBe('(file not specified)')
    expect(groups[0]!.errors).toHaveLength(2)
  })

  test('deduplicates suggestion / docLink pairs inside a group', () => {
    const errors: ValidationError[] = [
      { file: 'a.json', message: 'x', suggestion: 'retry', docLink: 'https://e/' },
      { file: 'a.json', message: 'y', suggestion: 'retry', docLink: 'https://e/' },
      { file: 'a.json', message: 'z', suggestion: 'check config' },
    ]
    const group = groupValidationErrors(errors)[0]!
    expect(group.suggestionPairs).toHaveLength(2)
    expect(group.suggestionPairs).toEqual(
      expect.arrayContaining([
        { suggestion: 'retry', docLink: 'https://e/' },
        { suggestion: 'check config', docLink: undefined },
      ]),
    )
  })

  test('sorts path-less errors before keyed paths inside a group', () => {
    const errors: ValidationError[] = [
      { file: 'a.json', path: 'z', message: 'last' },
      { file: 'a.json', message: 'root' },
      { file: 'a.json', path: 'a', message: 'first' },
    ]
    const group = groupValidationErrors(errors)[0]!
    expect(group.errors.map(e => e.message)).toEqual(['root', 'first', 'last'])
  })
})
