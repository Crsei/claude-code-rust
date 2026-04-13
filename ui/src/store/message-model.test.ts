import { describe, expect, test } from 'bun:test'
import { buildRenderItems, type RawMessage } from './message-model.js'

function toolUse(id: string, name: string, input: any, timestamp: number): RawMessage {
  return {
    id,
    role: 'tool_use',
    content: `Tool: ${name}`,
    timestamp,
    toolName: name,
    toolInput: input,
    toolUseId: id,
  }
}

function toolResult(
  toolUseId: string,
  output: string,
  timestamp: number,
  isError = false,
): RawMessage {
  return {
    id: `result-${toolUseId}-${timestamp}`,
    role: 'tool_result',
    content: output,
    timestamp,
    toolUseId,
    isError,
  }
}

describe('buildRenderItems', () => {
  test('pairs live tool use and tool result into one activity', () => {
    const items = buildRenderItems([
      toolUse('tool-1', 'Read', { file_path: '/tmp/file.ts' }, 1),
      toolResult('tool-1', '{"output":"done"}', 2),
    ], {
      viewMode: 'prompt',
      isBusy: false,
    })

    expect(items).toHaveLength(1)
    expect(items[0]?.type).toBe('tool_activity')
    if (items[0]?.type === 'tool_activity') {
      expect(items[0].name).toBe('Read')
      expect(items[0].status).toBe('success')
      expect(items[0].inputSummary).toContain('/tmp/file.ts')
      expect(items[0].outputSummary).toContain('done')
    }
  })

  test('reconstructs tool timeline from conversation replay content blocks', () => {
    const items = buildRenderItems([
      {
        id: 'assistant-1',
        role: 'assistant',
        content: '',
        timestamp: 1,
        contentBlocks: [
          { type: 'tool_use', id: 'tool-1', name: 'Read', input: { file_path: '/tmp/file.ts' } },
          { type: 'text', text: 'Summarizing file' },
        ],
      },
      {
        id: 'user-1',
        role: 'user',
        content: '',
        timestamp: 2,
        contentBlocks: [
          {
            type: 'tool_result',
            tool_use_id: 'tool-1',
            content: 'export const value = 1',
          },
        ],
      },
    ], {
      viewMode: 'transcript',
      isBusy: false,
    })

    expect(items.some(item => item.type === 'tool_activity')).toBe(true)
    expect(items.some(item => item.type === 'assistant_text')).toBe(true)

    const activity = items.find(item => item.type === 'tool_activity')
    if (activity?.type === 'tool_activity') {
      expect(activity.status).toBe('success')
      expect(activity.outputSummary).toContain('export const value = 1')
    }
  })

  test('groups only contiguous read/search tools in prompt view', () => {
    const promptItems = buildRenderItems([
      toolUse('tool-1', 'Read', { file_path: '/tmp/a.ts' }, 1),
      toolResult('tool-1', 'A', 2),
      toolUse('tool-2', 'Glob', { path: '/tmp', pattern: '*.ts' }, 3),
      toolResult('tool-2', 'B', 4),
      toolUse('tool-3', 'Bash', { command: 'pwd' }, 5),
      toolResult('tool-3', 'C', 6),
    ], {
      viewMode: 'prompt',
      isBusy: false,
    })

    expect(promptItems[0]?.type).toBe('tool_group')
    expect(promptItems[1]?.type).toBe('tool_activity')
    if (promptItems[0]?.type === 'tool_group') {
      expect(promptItems[0].activities).toHaveLength(2)
    }

    const transcriptItems = buildRenderItems([
      toolUse('tool-1', 'Read', { file_path: '/tmp/a.ts' }, 1),
      toolResult('tool-1', 'A', 2),
      toolUse('tool-2', 'Glob', { path: '/tmp', pattern: '*.ts' }, 3),
      toolResult('tool-2', 'B', 4),
    ], {
      viewMode: 'transcript',
      isBusy: false,
    })

    expect(transcriptItems.every(item => item.type !== 'tool_group')).toBe(true)
  })
})
