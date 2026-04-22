import { describe, expect, test } from 'bun:test'
import { extractFileEditContext } from '../file-edit.js'

describe('extractFileEditContext', () => {
  test('extracts a single Edit with file_path + old_string + new_string', () => {
    const ctx = extractFileEditContext('Edit', {
      file_path: '/tmp/foo.ts',
      old_string: 'a',
      new_string: 'b',
    })
    expect(ctx).toEqual({
      kind: 'edit',
      filePath: '/tmp/foo.ts',
      edits: [{ oldString: 'a', newString: 'b' }],
    })
  })

  test('preserves replace_all when provided', () => {
    const ctx = extractFileEditContext('Edit', {
      file_path: '/tmp/foo.ts',
      old_string: 'old',
      new_string: 'new',
      replace_all: true,
    })
    expect(ctx?.edits[0]!.replaceAll).toBe(true)
  })

  test('extracts MultiEdit with an edits array', () => {
    const ctx = extractFileEditContext('MultiEdit', {
      file_path: '/tmp/foo.ts',
      edits: [
        { old_string: 'a', new_string: 'b' },
        { old_string: 'c', new_string: 'd', replace_all: true },
      ],
    })
    expect(ctx?.kind).toBe('multi_edit')
    expect(ctx?.edits).toHaveLength(2)
    expect(ctx?.edits[1]!.replaceAll).toBe(true)
  })

  test('treats Write with content as a file-write', () => {
    const ctx = extractFileEditContext('Write', {
      file_path: '/tmp/new.txt',
      content: 'hello\nworld',
    })
    expect(ctx).toEqual({
      kind: 'write',
      filePath: '/tmp/new.txt',
      edits: [{ oldString: '', newString: 'hello\nworld' }],
    })
  })

  test('returns null when file_path is missing', () => {
    expect(
      extractFileEditContext('Edit', {
        old_string: 'a',
        new_string: 'b',
      }),
    ).toBeNull()
  })

  test('returns null for non-edit tool inputs', () => {
    expect(extractFileEditContext('Bash', { command: 'ls' })).toBeNull()
    expect(extractFileEditContext('Edit', null)).toBeNull()
    expect(extractFileEditContext('Edit', 'not an object')).toBeNull()
  })

  test('drops malformed entries in a MultiEdit array but keeps the valid ones', () => {
    const ctx = extractFileEditContext('MultiEdit', {
      file_path: '/tmp/x.ts',
      edits: [
        { old_string: 'ok', new_string: 'ok2' },
        { old_string: 123 }, // malformed
        { old_string: 'second', new_string: 'second2' },
      ],
    })
    expect(ctx?.edits).toHaveLength(2)
    expect(ctx?.edits.map(e => e.oldString)).toEqual(['ok', 'second'])
  })
})
