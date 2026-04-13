import type {
  ConversationMessage,
  FrontendContentBlock,
  ToolResultContent,
} from '../ipc/protocol.js'
import type { ViewMode } from '../keybindings.js'
import { truncate } from '../utils.js'

export type RawMessageRole =
  | ConversationMessage['role']
  | 'tool_use'
  | 'tool_result'

export interface RawMessage {
  id: string
  role: RawMessageRole
  content: string
  timestamp: number
  contentBlocks?: FrontendContentBlock[]
  costUsd?: number
  thinking?: string
  level?: string
  toolName?: string
  toolInput?: any
  toolUseId?: string
  isError?: boolean
}

export type ToolActivityStatus =
  | 'pending'
  | 'running'
  | 'success'
  | 'error'
  | 'cancelled'

export interface UserTextRenderItem {
  type: 'user_text'
  id: string
  content: string
  timestamp: number
}

export interface AssistantTextRenderItem {
  type: 'assistant_text'
  id: string
  content: string
  thinking?: string
  timestamp: number
  costUsd?: number
}

export interface SystemTextRenderItem {
  type: 'system_text'
  id: string
  content: string
  timestamp: number
  level?: string
}

export interface ToolActivityRenderItem {
  type: 'tool_activity'
  id: string
  toolUseId: string
  name: string
  input: any
  inputSummary: string
  output?: string
  outputSummary?: string
  status: ToolActivityStatus
  isError: boolean
  timestamp: number
  resultTimestamp?: number
}

export interface ToolGroupRenderItem {
  type: 'tool_group'
  id: string
  label: string
  status: ToolActivityStatus
  timestamp: number
  activities: ToolActivityRenderItem[]
  latestSummary?: string
}

export interface OrphanToolResultRenderItem {
  type: 'tool_result_orphan'
  id: string
  toolUseId: string
  output: string
  outputSummary: string
  status: ToolActivityStatus
  isError: boolean
  timestamp: number
}

export interface StreamingRenderItem {
  type: 'streaming'
  id: string
  content: string
  thinking?: string
  timestamp: number
}

export type RenderItem =
  | UserTextRenderItem
  | AssistantTextRenderItem
  | SystemTextRenderItem
  | ToolActivityRenderItem
  | ToolGroupRenderItem
  | OrphanToolResultRenderItem
  | StreamingRenderItem

const GROUPABLE_TOOL_NAMES = new Set(['Read', 'Glob', 'Grep'])

export function conversationToRawMessage(message: ConversationMessage): RawMessage {
  return {
    id: message.id,
    role: message.role,
    content: message.content,
    timestamp: message.timestamp,
    contentBlocks: message.content_blocks,
    costUsd: message.cost_usd,
    thinking: message.thinking,
    level: message.level,
  }
}

export function buildRenderItems(
  rawMessages: RawMessage[],
  options: {
    viewMode: ViewMode
    isBusy: boolean
    streamingText?: string
    streamingThinking?: string
  },
): RenderItem[] {
  const items: RenderItem[] = []
  const toolIndices = new Map<string, number>()
  const standaloneToolUseIds = new Set(
    rawMessages
      .filter(message => message.role === 'tool_use')
      .map(message => message.id),
  )
  const standaloneToolResultIds = new Set(
    rawMessages
      .filter(
        (message): message is RawMessage & { toolUseId: string } =>
          message.role === 'tool_result' && typeof message.toolUseId === 'string',
      )
      .map(message => message.toolUseId),
  )

  const pushAssistantSegment = (
    raw: RawMessage,
    segmentIndex: number,
    text: string,
    thinking?: string,
  ) => {
    if (!text && !thinking) {
      return
    }
    items.push({
      type: 'assistant_text',
      id: `${raw.id}:assistant:${segmentIndex}`,
      content: text,
      thinking,
      timestamp: raw.timestamp,
      costUsd: raw.costUsd,
    })
  }

  const pushUserSegment = (raw: RawMessage, segmentIndex: number, text: string) => {
    if (!text) {
      return
    }
    items.push({
      type: 'user_text',
      id: `${raw.id}:user:${segmentIndex}`,
      content: text,
      timestamp: raw.timestamp,
    })
  }

  const pushToolUse = (
    toolUseId: string,
    name: string,
    input: any,
    timestamp: number,
    source: 'live' | 'embedded',
  ) => {
    if (source === 'embedded' && standaloneToolUseIds.has(toolUseId)) {
      return
    }
    const existingIndex = toolIndices.get(toolUseId)
    if (existingIndex !== undefined) {
      const current = items[existingIndex]
      if (current?.type === 'tool_activity') {
        current.name = current.name || name
        current.input = current.input ?? input
        current.inputSummary = current.inputSummary || summarizeToolInput(input)
      }
      return
    }

    const activity: ToolActivityRenderItem = {
      type: 'tool_activity',
      id: `tool:${toolUseId}`,
      toolUseId,
      name: name || 'Unknown Tool',
      input,
      inputSummary: summarizeToolInput(input),
      status: 'pending',
      isError: false,
      timestamp,
    }
    toolIndices.set(toolUseId, items.length)
    items.push(activity)
  }

  const applyToolResult = (
    toolUseId: string,
    output: string,
    isError: boolean,
    timestamp: number,
    source: 'live' | 'embedded',
  ) => {
    if (source === 'embedded' && standaloneToolResultIds.has(toolUseId)) {
      return
    }

    const status = classifyToolResultStatus(output, isError)
    const outputSummary = summarizeToolOutput(output)
    const existingIndex = toolIndices.get(toolUseId)

    if (existingIndex === undefined) {
      items.push({
        type: 'tool_result_orphan',
        id: `tool-result-orphan:${toolUseId}:${timestamp}`,
        toolUseId,
        output,
        outputSummary,
        status,
        isError,
        timestamp,
      })
      return
    }

    const current = items[existingIndex]
    if (current?.type !== 'tool_activity') {
      return
    }
    current.output = output
    current.outputSummary = outputSummary
    current.status = status
    current.isError = isError || status === 'error'
    current.resultTimestamp = timestamp
  }

  for (const raw of rawMessages) {
    switch (raw.role) {
      case 'tool_use':
        pushToolUse(raw.id, raw.toolName ?? 'Unknown Tool', raw.toolInput, raw.timestamp, 'live')
        break
      case 'tool_result':
        applyToolResult(
          raw.toolUseId ?? raw.id,
          raw.content,
          raw.isError ?? false,
          raw.timestamp,
          'live',
        )
        break
      case 'assistant':
        if (raw.contentBlocks?.length) {
          let textParts: string[] = []
          let thinkingParts: string[] = []
          let segmentIndex = 0
          const flush = () => {
            pushAssistantSegment(
              raw,
              segmentIndex++,
              textParts.join('\n').trim(),
              joinParts(thinkingParts),
            )
            textParts = []
            thinkingParts = []
          }

          for (const block of raw.contentBlocks) {
            switch (block.type) {
              case 'text':
                textParts.push(block.text)
                break
              case 'thinking':
                thinkingParts.push(block.thinking)
                break
              case 'redacted_thinking':
                thinkingParts.push('[redacted thinking]')
                break
              case 'tool_use':
                flush()
                pushToolUse(block.id, block.name, block.input, raw.timestamp, 'embedded')
                break
              default:
                break
            }
          }

          flush()
          break
        }

        pushAssistantSegment(raw, 0, raw.content, raw.thinking)
        break
      case 'user':
        if (raw.contentBlocks?.length) {
          let textParts: string[] = []
          let segmentIndex = 0
          let renderedText = false
          const flush = () => {
            const segment = textParts.join('\n').trim()
            if (segment) {
              renderedText = true
              pushUserSegment(raw, segmentIndex++, segment)
            }
            textParts = []
          }

          for (const block of raw.contentBlocks) {
            switch (block.type) {
              case 'text':
                textParts.push(block.text)
                break
              case 'tool_result':
                flush()
                applyToolResult(
                  block.tool_use_id,
                  toolResultContentToString(block.content),
                  !!block.is_error,
                  raw.timestamp,
                  'embedded',
                )
                break
              default:
                break
            }
          }

          flush()
          if (!renderedText && !raw.contentBlocks.some(block => block.type === 'tool_result')) {
            pushUserSegment(raw, segmentIndex, raw.content)
          }
          break
        }

        pushUserSegment(raw, 0, raw.content)
        break
      case 'system':
        if (raw.content) {
          items.push({
            type: 'system_text',
            id: raw.id,
            content: raw.content,
            timestamp: raw.timestamp,
            level: raw.level,
          })
        }
        break
    }
  }

  for (const item of items) {
    if (item.type === 'tool_activity' && item.status === 'pending') {
      item.status = options.isBusy ? 'running' : 'pending'
    }
  }

  const rendered =
    options.viewMode === 'prompt'
      ? groupToolActivities(items)
      : items

  if (options.streamingText || options.streamingThinking) {
    rendered.push({
      type: 'streaming',
      id: 'streaming-partial',
      content: options.streamingText ?? '',
      thinking: options.streamingThinking || undefined,
      timestamp: Date.now(),
    })
  }

  return rendered
}

function joinParts(parts: string[]): string | undefined {
  const joined = parts
    .map(part => part.trim())
    .filter(Boolean)
    .join('\n')
  return joined || undefined
}

function groupToolActivities(items: RenderItem[]): RenderItem[] {
  const grouped: RenderItem[] = []
  let buffer: ToolActivityRenderItem[] = []

  const flush = () => {
    if (buffer.length === 0) {
      return
    }
    if (buffer.length === 1) {
      grouped.push(buffer[0])
    } else {
      grouped.push(buildToolGroup(buffer))
    }
    buffer = []
  }

  for (const item of items) {
    if (item.type === 'tool_activity' && GROUPABLE_TOOL_NAMES.has(item.name)) {
      buffer.push(item)
      continue
    }
    flush()
    grouped.push(item)
  }

  flush()
  return grouped
}

function buildToolGroup(activities: ToolActivityRenderItem[]): ToolGroupRenderItem {
  const counts = new Map<string, number>()
  for (const activity of activities) {
    counts.set(activity.name, (counts.get(activity.name) ?? 0) + 1)
  }

  const label = [...counts.entries()]
    .map(([name, count]) => `${name} ${count}`)
    .join(', ')

  return {
    type: 'tool_group',
    id: `tool-group:${activities[0]!.toolUseId}:${activities.length}`,
    label,
    status: mergeStatuses(activities.map(activity => activity.status)),
    timestamp: activities[0]!.timestamp,
    activities,
    latestSummary: activities[activities.length - 1]?.inputSummary,
  }
}

function mergeStatuses(statuses: ToolActivityStatus[]): ToolActivityStatus {
  if (statuses.some(status => status === 'error')) {
    return 'error'
  }
  if (statuses.some(status => status === 'running')) {
    return 'running'
  }
  if (statuses.some(status => status === 'pending')) {
    return 'pending'
  }
  if (statuses.some(status => status === 'cancelled')) {
    return 'cancelled'
  }
  return 'success'
}

function summarizeToolInput(input: any): string {
  if (typeof input === 'string') {
    return compactLine(input)
  }
  if (!input || typeof input !== 'object') {
    return compactLine(String(input ?? ''))
  }

  if (typeof input.command === 'string' && input.command.trim()) {
    return compactLine(input.command)
  }
  if (typeof input.file_path === 'string' && input.file_path.trim()) {
    return compactLine(input.file_path)
  }
  if (typeof input.url === 'string' && input.url.trim()) {
    return compactLine(input.url)
  }
  if (typeof input.pattern === 'string' && input.pattern.trim()) {
    const path = typeof input.path === 'string' && input.path.trim()
      ? ` in ${input.path}`
      : ''
    return compactLine(`"${input.pattern}"${path}`)
  }
  if (typeof input.path === 'string' && input.path.trim()) {
    return compactLine(input.path)
  }
  if (typeof input.prompt === 'string' && input.prompt.trim()) {
    return compactLine(input.prompt)
  }

  try {
    return compactLine(JSON.stringify(input))
  } catch {
    return '(structured input)'
  }
}

function summarizeToolOutput(output: string): string {
  const extracted = extractOutputFromJson(output)
  if (extracted) {
    return compactLine(extracted)
  }

  const firstNonEmptyLine = output
    .split(/\r?\n/)
    .map(line => line.trim())
    .find(Boolean)

  return compactLine(firstNonEmptyLine || output.trim() || '(no output)')
}

function extractOutputFromJson(output: string): string | undefined {
  const trimmed = output.trim()
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) {
    return undefined
  }

  try {
    const parsed = JSON.parse(trimmed)
    const preferred = [parsed?.output, parsed?.stdout, parsed?.stderr, parsed?.error]
      .find(value => typeof value === 'string' && value.trim()) as string | undefined
    if (preferred) {
      return preferred
    }
    return typeof parsed === 'string' ? parsed : JSON.stringify(parsed)
  } catch {
    return undefined
  }
}

function classifyToolResultStatus(
  output: string,
  isError: boolean,
): ToolActivityStatus {
  const lowered = output.toLowerCase()
  if (lowered.includes('interrupted by user') || lowered.includes('cancelled') || lowered.includes('aborted')) {
    return 'cancelled'
  }
  if (isError) {
    return 'error'
  }
  return 'success'
}

function toolResultContentToString(content: ToolResultContent): string {
  if (typeof content === 'string') {
    return content
  }

  return content
    .map(block => {
      switch (block.type) {
        case 'text':
          return block.text
        case 'thinking':
          return block.thinking
        case 'tool_result':
          return toolResultContentToString(block.content)
        default:
          return ''
      }
    })
    .filter(Boolean)
    .join('\n')
}

function compactLine(value: string): string {
  const compact = value.replace(/\s+/g, ' ').trim()
  return truncate(compact, 120)
}
