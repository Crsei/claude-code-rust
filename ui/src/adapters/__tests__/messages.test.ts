import { describe, expect, test } from 'bun:test'
import type { ConversationMessage } from '../../ipc/protocol.js'
import type { RawMessage } from '../../store/message-model.js'
import {
  mapConversationMessageToViewModels,
  mapRawMessageToViewModels,
} from '../messages.js'

describe('mapRawMessageToViewModels', () => {
  test('maps a plain user text message', () => {
    const raw: RawMessage = {
      id: 'u-1',
      role: 'user',
      content: 'hello there',
      timestamp: 1,
    }
    const items = mapRawMessageToViewModels(raw)
    expect(items).toHaveLength(1)
    expect(items[0]!.kind).toBe('user_text')
    if (items[0]!.kind === 'user_text') {
      expect(items[0]!.text).toBe('hello there')
      expect(items[0]!.id).toBe('u-1:user:0')
    }
  })

  test('maps assistant text + thinking into a single segment', () => {
    const raw: RawMessage = {
      id: 'a-1',
      role: 'assistant',
      content: '',
      timestamp: 5,
      contentBlocks: [
        { type: 'thinking', thinking: 'planning…' },
        { type: 'text', text: 'Here is the answer.' },
      ],
      costUsd: 0.01,
    }

    const items = mapRawMessageToViewModels(raw)
    expect(items).toHaveLength(1)
    expect(items[0]!.kind).toBe('assistant_message')
    if (items[0]!.kind === 'assistant_message') {
      expect(items[0]!.costUsd).toBe(0.01)
      expect(items[0]!.segments).toHaveLength(1)
      expect(items[0]!.segments[0]!.text).toBe('Here is the answer.')
      expect(items[0]!.segments[0]!.thinking).toBe('planning…')
      expect(items[0]!.segments[0]!.redactedThinking).toBeUndefined()
    }
  })

  test('splits assistant segments on tool_use boundaries and emits tool_use view models', () => {
    const raw: RawMessage = {
      id: 'a-2',
      role: 'assistant',
      content: '',
      timestamp: 10,
      contentBlocks: [
        { type: 'text', text: 'Let me read that file.' },
        {
          type: 'tool_use',
          id: 'tool-1',
          name: 'Read',
          input: { file_path: '/tmp/file.ts' },
        },
        { type: 'text', text: 'Here is what I found.' },
      ],
    }

    const items = mapRawMessageToViewModels(raw)
    expect(items).toHaveLength(2)
    const assistant = items.find(item => item.kind === 'assistant_message')
    const tool = items.find(item => item.kind === 'tool_use')
    expect(assistant).toBeDefined()
    expect(tool).toBeDefined()
    if (assistant?.kind === 'assistant_message') {
      expect(assistant.segments).toHaveLength(2)
      expect(assistant.segments[0]!.text).toBe('Let me read that file.')
      expect(assistant.segments[1]!.text).toBe('Here is what I found.')
    }
    if (tool?.kind === 'tool_use') {
      expect(tool.name).toBe('Read')
      expect(tool.toolUseId).toBe('tool-1')
      expect(tool.inputDetail).toBe('/tmp/file.ts')
      expect(tool.status).toBe('pending')
    }
  })

  test('marks thinking segments whose source was redacted', () => {
    const raw: RawMessage = {
      id: 'a-3',
      role: 'assistant',
      content: '',
      timestamp: 12,
      contentBlocks: [
        { type: 'redacted_thinking', data: 'opaque' },
        { type: 'text', text: 'Ok.' },
      ],
    }
    const items = mapRawMessageToViewModels(raw)
    expect(items).toHaveLength(1)
    if (items[0]!.kind === 'assistant_message') {
      const segment = items[0]!.segments[0]!
      expect(segment.thinking).toBe('[redacted thinking]')
      expect(segment.redactedThinking).toBe(true)
    }
  })

  test('surfaces user image blocks as their own view models', () => {
    const raw: RawMessage = {
      id: 'u-2',
      role: 'user',
      content: '',
      timestamp: 20,
      contentBlocks: [
        { type: 'text', text: 'look at this' },
        {
          type: 'image',
          source: { type: 'base64', media_type: 'image/png', data: 'AAAA' },
        },
      ],
    }
    const items = mapRawMessageToViewModels(raw)
    expect(items).toHaveLength(2)
    expect(items[0]!.kind).toBe('user_text')
    expect(items[1]!.kind).toBe('user_image')
    if (items[1]!.kind === 'user_image') {
      expect(items[1]!.image.data).toBe('AAAA')
      expect(items[1]!.image.mediaType).toBe('image/png')
    }
  })

  test('normalizes tool_result embedded in a user replay message', () => {
    const raw: RawMessage = {
      id: 'u-3',
      role: 'user',
      content: '',
      timestamp: 30,
      contentBlocks: [
        {
          type: 'tool_result',
          tool_use_id: 'tool-9',
          content: 'done',
        },
      ],
    }
    const items = mapRawMessageToViewModels(raw)
    expect(items).toHaveLength(1)
    expect(items[0]!.kind).toBe('tool_result')
    if (items[0]!.kind === 'tool_result') {
      expect(items[0]!.toolUseId).toBe('tool-9')
      expect(items[0]!.content.text).toBe('done')
      expect(items[0]!.status).toBe('success')
      expect(items[0]!.isError).toBe(false)
    }
  })

  test('classifies cancelled tool results when the output mentions interruption', () => {
    const raw: RawMessage = {
      id: 'r-1',
      role: 'tool_result',
      content: 'Interrupted by user',
      timestamp: 40,
      toolUseId: 'tool-5',
    }
    const items = mapRawMessageToViewModels(raw)
    expect(items[0]!.kind).toBe('tool_result')
    if (items[0]!.kind === 'tool_result') {
      expect(items[0]!.status).toBe('cancelled')
      expect(items[0]!.isError).toBe(false)
    }
  })

  test('maps system messages with recognized levels', () => {
    const raw: RawMessage = {
      id: 's-1',
      role: 'system',
      content: 'retrying...',
      timestamp: 50,
      level: 'WARNING',
    }
    const items = mapRawMessageToViewModels(raw)
    expect(items).toHaveLength(1)
    expect(items[0]!.kind).toBe('system_info')
    if (items[0]!.kind === 'system_info') {
      expect(items[0]!.level).toBe('warning')
    }
  })

  test('drops system messages that carry no content', () => {
    const raw: RawMessage = {
      id: 's-2',
      role: 'system',
      content: '',
      timestamp: 51,
    }
    const items = mapRawMessageToViewModels(raw)
    expect(items).toEqual([])
  })

  test('round-trips a ConversationMessage through the conversation adapter', () => {
    const message: ConversationMessage = {
      id: 'c-1',
      role: 'assistant',
      content: '',
      timestamp: 60,
      content_blocks: [
        { type: 'text', text: 'streamed reply' },
      ],
      cost_usd: 0.5,
    }
    const items = mapConversationMessageToViewModels(message)
    expect(items).toHaveLength(1)
    expect(items[0]!.kind).toBe('assistant_message')
    if (items[0]!.kind === 'assistant_message') {
      expect(items[0]!.costUsd).toBe(0.5)
      expect(items[0]!.segments[0]!.text).toBe('streamed reply')
    }
  })
})
