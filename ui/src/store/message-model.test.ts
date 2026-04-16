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
      toolUse('tool-4', 'Read', { file_path: '/tmp/very/long/path/that/should/still/be/visible/in/transcript/view.ts' }, 5),
      toolResult('tool-4', 'D', 6),
      toolUse('tool-5', 'Read', { file_path: '/tmp/final.ts' }, 7),
      toolResult('tool-5', 'E', 8),
      toolUse('tool-3', 'Bash', { command: 'pwd' }, 9),
      toolResult('tool-3', 'C', 10),
    ], {
      viewMode: 'prompt',
      isBusy: false,
    })

    expect(promptItems[0]?.type).toBe('tool_group')
    expect(promptItems[1]?.type).toBe('tool_activity')
    if (promptItems[0]?.type === 'tool_group') {
      expect(promptItems[0].title).toBe('Read 3 files, Glob 1 pattern')
      expect(promptItems[0].activities).toHaveLength(4)
      expect(promptItems[0].previewLines).toEqual([
        'Glob("*.ts" in /tmp)',
        'Read(/tmp/very/long/path/that/should/still/be/visible/in/transcript/view.ts)',
        'Read(/tmp/final.ts)',
      ])
      expect(promptItems[0].hiddenCount).toBe(1)
    }

    const transcriptItems = buildRenderItems([
      toolUse('tool-1', 'Read', { file_path: '/tmp/very/long/path/that/should/still/be/visible/in/transcript/view.ts' }, 1),
      toolResult('tool-1', 'A', 2),
      toolUse('tool-2', 'Glob', { path: '/tmp', pattern: '*.ts' }, 3),
      toolResult('tool-2', 'B', 4),
    ], {
      viewMode: 'transcript',
      isBusy: false,
    })

    expect(transcriptItems.every(item => item.type !== 'tool_group')).toBe(true)
    const transcriptActivity = transcriptItems.find(item => item.type === 'tool_activity')
    if (transcriptActivity?.type === 'tool_activity') {
      expect(transcriptActivity.inputDetail).toBe('/tmp/very/long/path/that/should/still/be/visible/in/transcript/view.ts')
    }
  })

  test('formats AskUserQuestion input as plain question text instead of json', () => {
    const question = 'Subagent call failed (Azure: DeploymentNotFound). Do you want A) implement Gomoku in the main agent, or B) fix or switch the deployment first and then retry the subagent?'
    const items = buildRenderItems([
      toolUse('tool-q', 'AskUserQuestion', { question }, 1),
    ], {
      viewMode: 'transcript',
      isBusy: true,
    })

    expect(items).toHaveLength(1)
    expect(items[0]?.type).toBe('tool_activity')
    if (items[0]?.type === 'tool_activity') {
      expect(items[0].name).toBe('AskUserQuestion')
      expect(items[0].inputDetail).toBe(question)
      expect(items[0].inputSummary).toContain('DeploymentNotFound')
      expect(items[0].inputDetail).not.toContain('{"question"')
      expect(items[0].inputSummary).not.toContain('{"question"')
    }
  })
})
