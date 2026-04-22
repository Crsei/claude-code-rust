import { describe, expect, test } from 'bun:test'
import {
  inlineBlocksFromContent,
  normalizeToolResultBlocks,
  normalizeToolResultContent,
} from '../content-blocks.js'

describe('inlineBlocksFromContent', () => {
  test('flattens text, thinking, and image blocks in order', () => {
    const blocks = inlineBlocksFromContent([
      { type: 'text', text: 'hello' },
      { type: 'thinking', thinking: 'weighing options' },
      {
        type: 'image',
        source: { type: 'base64', media_type: 'image/jpeg', data: 'ZZZ' },
      },
    ])

    expect(blocks).toHaveLength(3)
    expect(blocks[0]).toEqual({ kind: 'text', text: 'hello' })
    expect(blocks[1]).toEqual({
      kind: 'thinking',
      text: 'weighing options',
      redacted: false,
    })
    expect(blocks[2]).toEqual({
      kind: 'image',
      image: { data: 'ZZZ', mediaType: 'image/jpeg' },
    })
  })

  test('flags redacted_thinking entries', () => {
    const blocks = inlineBlocksFromContent([
      { type: 'redacted_thinking', data: 'x' },
    ])
    expect(blocks).toHaveLength(1)
    expect(blocks[0]).toEqual({
      kind: 'thinking',
      text: '[redacted thinking]',
      redacted: true,
    })
  })

  test('ignores tool_use and tool_result inline blocks', () => {
    const blocks = inlineBlocksFromContent([
      { type: 'text', text: 'before' },
      { type: 'tool_use', id: 't-1', name: 'Read', input: {} },
      { type: 'tool_result', tool_use_id: 't-1', content: 'done' },
      { type: 'text', text: 'after' },
    ])
    expect(blocks.map(block => block.kind)).toEqual(['text', 'text'])
  })
})

describe('normalizeToolResultContent', () => {
  test('returns the string body when content is a plain string', () => {
    expect(normalizeToolResultContent('stdout line')).toEqual({
      text: 'stdout line',
      images: [],
    })
  })

  test('extracts text and images from nested blocks', () => {
    const result = normalizeToolResultContent([
      { type: 'text', text: 'line 1' },
      {
        type: 'image',
        source: { type: 'base64', media_type: 'image/png', data: 'AAAA' },
      },
      { type: 'text', text: 'line 2' },
    ])

    expect(result.text).toBe('line 1\nline 2')
    expect(result.images).toEqual([{ data: 'AAAA', mediaType: 'image/png' }])
  })

  test('recurses into nested tool_result content', () => {
    const result = normalizeToolResultContent([
      {
        type: 'tool_result',
        tool_use_id: 't-2',
        content: [
          { type: 'text', text: 'inner' },
          {
            type: 'image',
            source: { type: 'base64', media_type: 'image/png', data: 'XX' },
          },
        ],
      },
    ])
    expect(result.text).toBe('inner')
    expect(result.images).toEqual([{ data: 'XX', mediaType: 'image/png' }])
  })
})

describe('normalizeToolResultBlocks', () => {
  test('returns the raw output when no structured blocks are present', () => {
    expect(normalizeToolResultBlocks('raw out', undefined)).toEqual({
      text: 'raw out',
      images: [],
    })
  })

  test('prefers structured text and merges image blocks', () => {
    const result = normalizeToolResultBlocks('fallback', [
      { type: 'text', text: 'structured' },
      {
        type: 'image',
        media_type: 'image/png',
        size_bytes: 128,
        data: 'BBBB',
      },
    ])

    expect(result.text).toBe('structured')
    expect(result.images).toEqual([
      { data: 'BBBB', mediaType: 'image/png', sizeBytes: 128 },
    ])
  })

  test('falls back to raw output when structured text is empty', () => {
    const result = normalizeToolResultBlocks('fallback text', [
      {
        type: 'image',
        media_type: 'image/png',
        data: 'CCCC',
      },
    ])
    expect(result.text).toBe('fallback text')
    expect(result.images).toEqual([{ data: 'CCCC', mediaType: 'image/png' }])
  })

  test('drops image blocks without base64 payload', () => {
    const result = normalizeToolResultBlocks('x', [
      { type: 'image', media_type: 'image/png' },
    ])
    expect(result.images).toEqual([])
  })
})
