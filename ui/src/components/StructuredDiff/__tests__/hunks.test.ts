import { describe, expect, test } from 'bun:test'
import {
  hunkFromEdit,
  hunkGutterWidth,
  parseUnifiedDiff,
} from '../hunks.js'

describe('parseUnifiedDiff', () => {
  test('returns an empty array for empty input', () => {
    expect(parseUnifiedDiff('')).toEqual([])
    expect(parseUnifiedDiff('no header here')).toEqual([])
  })

  test('parses a single-hunk unified diff with full line ranges', () => {
    const diff = [
      '--- a/foo.ts',
      '+++ b/foo.ts',
      '@@ -1,3 +1,4 @@',
      ' a',
      '-b',
      '+b-new',
      ' c',
      '+d',
    ].join('\n')

    const hunks = parseUnifiedDiff(diff)
    expect(hunks).toHaveLength(1)
    expect(hunks[0]!.oldStart).toBe(1)
    expect(hunks[0]!.oldLines).toBe(3)
    expect(hunks[0]!.newStart).toBe(1)
    expect(hunks[0]!.newLines).toBe(4)
    expect(hunks[0]!.lines.map(l => l.kind)).toEqual([
      'context',
      'remove',
      'add',
      'context',
      'add',
    ])
    expect(hunks[0]!.lines.map(l => l.text)).toEqual([
      'a',
      'b',
      'b-new',
      'c',
      'd',
    ])
  })

  test('parses multiple hunks with short headers', () => {
    const diff = [
      '@@ -10 +10 @@',
      '-old',
      '+new',
      '@@ -20,2 +21,3 @@',
      ' ctx',
      '-drop',
      '+keep',
      '+extra',
    ].join('\n')
    const hunks = parseUnifiedDiff(diff)
    expect(hunks).toHaveLength(2)
    expect(hunks[0]!.oldLines).toBe(1) // default when ,count omitted
    expect(hunks[1]!.newStart).toBe(21)
    expect(hunks[1]!.newLines).toBe(3)
  })

  test('ignores "No newline" and stray file-header lines between hunks', () => {
    const diff = [
      '--- a',
      '+++ b',
      '@@ -1 +1 @@',
      '-x',
      '+y',
      '\\ No newline at end of file',
    ].join('\n')
    const hunks = parseUnifiedDiff(diff)
    expect(hunks).toHaveLength(1)
    expect(hunks[0]!.lines).toEqual([
      { kind: 'remove', text: 'x' },
      { kind: 'add', text: 'y' },
    ])
  })
})

describe('hunkFromEdit', () => {
  test('marks every old line - and every new line +', () => {
    const hunk = hunkFromEdit('a\nb', 'a\nc\nd')
    expect(hunk.oldLines).toBe(2)
    expect(hunk.newLines).toBe(3)
    expect(hunk.lines.map(l => l.kind)).toEqual([
      'remove',
      'remove',
      'add',
      'add',
      'add',
    ])
  })

  test('handles the empty old_string (file-write case)', () => {
    const hunk = hunkFromEdit('', 'hello\nworld')
    expect(hunk.oldLines).toBe(0)
    expect(hunk.newLines).toBe(2)
    expect(hunk.lines.every(l => l.kind === 'add')).toBe(true)
  })

  test('handles the empty new_string (full-delete case)', () => {
    const hunk = hunkFromEdit('gone', '')
    expect(hunk.oldLines).toBe(1)
    expect(hunk.newLines).toBe(0)
    expect(hunk.lines.every(l => l.kind === 'remove')).toBe(true)
  })
})

describe('hunkGutterWidth', () => {
  test('grows with the number of digits in the largest line number', () => {
    const smallHunk = hunkFromEdit('a', 'b')
    expect(hunkGutterWidth(smallHunk)).toBe(3) // "1" + marker + space

    const bigHunk = { ...smallHunk, oldStart: 999, newStart: 1000, oldLines: 1, newLines: 1 }
    expect(hunkGutterWidth(bigHunk)).toBe(6) // "1000" + marker + space
  })
})
