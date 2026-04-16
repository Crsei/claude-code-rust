import type {
  ConversationMessage,
  FrontendContentBlock,
  ToolResultContent,
} from '../ipc/protocol.js'
import type { ViewMode } from './app-store.js'

// Inlined from ui/src/utils.ts
function truncate(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text
  return text.slice(0, maxLen - 1) + '\u2026'
}

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
  inputDetail: string
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
  title: string
  status: ToolActivityStatus
  timestamp: number
  activities: ToolActivityRenderItem[]
  previewLines: string[]
  hiddenCount: number
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
const MAX_GROUP_PREVIEW_LINES = 3

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
    rawMessages.filter(m => m.role === 'tool_use').map(m => m.id),
  )
  const standaloneToolResultIds = new Set(
    rawMessages
      .filter((m): m is RawMessage & { toolUseId: string } =>
        m.role === 'tool_result' && typeof m.toolUseId === 'string')
      .map(m => m.toolUseId),
  )

  const pushAssistantSegment = (raw: RawMessage, segIdx: number, text: string, thinking?: string) => {
    if (!text && !thinking) return
    items.push({ type: 'assistant_text', id: `${raw.id}:assistant:${segIdx}`, content: text, thinking, timestamp: raw.timestamp, costUsd: raw.costUsd })
  }

  const pushUserSegment = (raw: RawMessage, segIdx: number, text: string) => {
    if (!text) return
    items.push({ type: 'user_text', id: `${raw.id}:user:${segIdx}`, content: text, timestamp: raw.timestamp })
  }

  const pushToolUse = (toolUseId: string, name: string, input: any, timestamp: number, source: 'live' | 'embedded') => {
    if (source === 'embedded' && standaloneToolUseIds.has(toolUseId)) return
    const existingIndex = toolIndices.get(toolUseId)
    if (existingIndex !== undefined) {
      const current = items[existingIndex]
      if (current?.type === 'tool_activity') {
        current.name = current.name || name
        current.input = current.input ?? input
        current.inputDetail = current.inputDetail || describeToolInput(input)
        current.inputSummary = current.inputSummary || summarizeToolInput(input)
      }
      return
    }
    const activity: ToolActivityRenderItem = {
      type: 'tool_activity', id: `tool:${toolUseId}`, toolUseId, name: name || 'Unknown Tool',
      input, inputDetail: describeToolInput(input), inputSummary: summarizeToolInput(input),
      status: 'pending', isError: false, timestamp,
    }
    toolIndices.set(toolUseId, items.length)
    items.push(activity)
  }

  const applyToolResult = (toolUseId: string, output: string, isError: boolean, timestamp: number, source: 'live' | 'embedded') => {
    if (source === 'embedded' && standaloneToolResultIds.has(toolUseId)) return
    const status = classifyToolResultStatus(output, isError)
    const outputSummary = summarizeToolOutput(output)
    const existingIndex = toolIndices.get(toolUseId)
    if (existingIndex === undefined) {
      items.push({ type: 'tool_result_orphan', id: `tool-result-orphan:${toolUseId}:${timestamp}`, toolUseId, output, outputSummary, status, isError, timestamp })
      return
    }
    const current = items[existingIndex]
    if (current?.type !== 'tool_activity') return
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
        applyToolResult(raw.toolUseId ?? raw.id, raw.content, raw.isError ?? false, raw.timestamp, 'live')
        break
      case 'assistant':
        if (raw.contentBlocks?.length) {
          let textParts: string[] = [], thinkingParts: string[] = [], segIdx = 0
          const flush = () => { pushAssistantSegment(raw, segIdx++, textParts.join('\n').trim(), joinParts(thinkingParts)); textParts = []; thinkingParts = [] }
          for (const block of raw.contentBlocks) {
            if (block.type === 'text') textParts.push(block.text)
            else if (block.type === 'thinking') thinkingParts.push(block.thinking)
            else if (block.type === 'redacted_thinking') thinkingParts.push('[redacted thinking]')
            else if (block.type === 'tool_use') { flush(); pushToolUse(block.id, block.name, block.input, raw.timestamp, 'embedded') }
          }
          flush()
        } else {
          pushAssistantSegment(raw, 0, raw.content, raw.thinking)
        }
        break
      case 'user':
        if (raw.contentBlocks?.length) {
          let textParts: string[] = [], segIdx = 0, renderedText = false
          const flush = () => { const s = textParts.join('\n').trim(); if (s) { renderedText = true; pushUserSegment(raw, segIdx++, s) } textParts = [] }
          for (const block of raw.contentBlocks) {
            if (block.type === 'text') textParts.push(block.text)
            else if (block.type === 'tool_result') { flush(); applyToolResult(block.tool_use_id, toolResultContentToString(block.content), !!block.is_error, raw.timestamp, 'embedded') }
          }
          flush()
          if (!renderedText && !raw.contentBlocks.some(b => b.type === 'tool_result')) pushUserSegment(raw, segIdx, raw.content)
        } else {
          pushUserSegment(raw, 0, raw.content)
        }
        break
      case 'system':
        if (raw.content) items.push({ type: 'system_text', id: raw.id, content: raw.content, timestamp: raw.timestamp, level: raw.level })
        break
    }
  }

  for (const item of items) {
    if (item.type === 'tool_activity' && item.status === 'pending') item.status = options.isBusy ? 'running' : 'pending'
  }

  const rendered = options.viewMode === 'prompt' ? groupToolActivities(items) : items
  if (options.streamingText || options.streamingThinking) {
    rendered.push({ type: 'streaming', id: 'streaming-partial', content: options.streamingText ?? '', thinking: options.streamingThinking || undefined, timestamp: Date.now() })
  }
  return rendered
}

function joinParts(parts: string[]): string | undefined {
  const joined = parts.map(p => p.trim()).filter(Boolean).join('\n')
  return joined || undefined
}

function groupToolActivities(items: RenderItem[]): RenderItem[] {
  const grouped: RenderItem[] = []
  let buffer: ToolActivityRenderItem[] = []
  const flush = () => { if (buffer.length === 0) return; grouped.push(buffer.length === 1 ? buffer[0] : buildToolGroup(buffer)); buffer = [] }
  for (const item of items) { if (item.type === 'tool_activity' && GROUPABLE_TOOL_NAMES.has(item.name)) { buffer.push(item); continue }; flush(); grouped.push(item) }
  flush()
  return grouped
}

function buildToolGroup(activities: ToolActivityRenderItem[]): ToolGroupRenderItem {
  const counts = new Map<string, number>()
  for (const a of activities) counts.set(a.name, (counts.get(a.name) ?? 0) + 1)
  const title = [...counts.entries()].map(([n, c]) => `${n} ${c} ${groupCountLabel(n, c)}`).join(', ')
  const preview = activities.slice(-MAX_GROUP_PREVIEW_LINES)
  return { type: 'tool_group', id: `tool-group:${activities[0]!.toolUseId}:${activities.length}`, title, status: mergeStatuses(activities.map(a => a.status)), timestamp: activities[0]!.timestamp, activities, previewLines: preview.map(formatToolGroupPreviewLine), hiddenCount: Math.max(0, activities.length - preview.length) }
}

function groupCountLabel(name: string, count: number): string {
  switch (name) { case 'Read': return count === 1 ? 'file' : 'files'; case 'Glob': return count === 1 ? 'pattern' : 'patterns'; case 'Grep': return count === 1 ? 'search' : 'searches'; default: return count === 1 ? 'call' : 'calls' }
}

function formatToolGroupPreviewLine(a: ToolActivityRenderItem): string { return compactLine(`${a.name}(${a.inputDetail || a.inputSummary || '(no input summary)'})`, 100) }
function mergeStatuses(statuses: ToolActivityStatus[]): ToolActivityStatus { if (statuses.some(s => s === 'error')) return 'error'; if (statuses.some(s => s === 'running')) return 'running'; if (statuses.some(s => s === 'pending')) return 'pending'; if (statuses.some(s => s === 'cancelled')) return 'cancelled'; return 'success' }

function describeToolInput(input: any): string {
  if (typeof input === 'string') return normalizeInline(input)
  if (!input || typeof input !== 'object') return normalizeInline(String(input ?? ''))
  if (typeof input.command === 'string' && input.command.trim()) return normalizeInline(input.command)
  if (typeof input.file_path === 'string' && input.file_path.trim()) return normalizeInline(input.file_path)
  if (typeof input.url === 'string' && input.url.trim()) return normalizeInline(input.url)
  if (typeof input.pattern === 'string' && input.pattern.trim()) { const path = typeof input.path === 'string' && input.path.trim() ? ` in ${input.path}` : ''; return normalizeInline(`"${input.pattern}"${path}`) }
  if (typeof input.path === 'string' && input.path.trim()) return normalizeInline(input.path)
  if (typeof input.prompt === 'string' && input.prompt.trim()) return normalizeInline(input.prompt)
  if (typeof input.question === 'string' && input.question.trim()) return normalizeInline(input.question)
  try { return normalizeInline(JSON.stringify(input)) } catch { return '(structured input)' }
}

function summarizeToolInput(input: any): string { return compactLine(describeToolInput(input)) }

function summarizeToolOutput(output: string): string {
  const extracted = extractOutputFromJson(output)
  if (extracted) return compactLine(extracted)
  const firstLine = output.split(/\r?\n/).map(l => l.trim()).find(Boolean)
  return compactLine(firstLine || output.trim() || '(no output)')
}

function extractOutputFromJson(output: string): string | undefined {
  const trimmed = output.trim()
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) return undefined
  try { const p = JSON.parse(trimmed); const pref = [p?.output, p?.stdout, p?.stderr, p?.error].find(v => typeof v === 'string' && v.trim()) as string | undefined; return pref ?? (typeof p === 'string' ? p : JSON.stringify(p)) } catch { return undefined }
}

function classifyToolResultStatus(output: string, isError: boolean): ToolActivityStatus {
  const l = output.toLowerCase()
  if (l.includes('interrupted by user') || l.includes('cancelled') || l.includes('aborted')) return 'cancelled'
  return isError ? 'error' : 'success'
}

function toolResultContentToString(content: ToolResultContent): string {
  if (typeof content === 'string') return content
  return content.map(b => { if (b.type === 'text') return b.text; if (b.type === 'thinking') return b.thinking; if (b.type === 'tool_result') return toolResultContentToString(b.content); return '' }).filter(Boolean).join('\n')
}

function normalizeInline(value: string): string { return value.replace(/\s+/g, ' ').trim() }
function compactLine(value: string, maxLength = 120): string { return truncate(normalizeInline(value), maxLength) }
