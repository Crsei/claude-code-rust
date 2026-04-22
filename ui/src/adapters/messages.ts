/**
 * Map from the current `RawMessage` / `ConversationMessage` shapes into
 * the normalized `MessageViewModel`.
 *
 * This is the primary adapter between the existing store
 * (`ui/src/store/message-model.ts`) and migration slices consuming
 * components under `ui/src/components/messages/**`.
 */

import type {
  ConversationMessage,
  FrontendContentBlock,
} from '../ipc/protocol.js'
import type { RawMessage } from '../store/message-model.js'
import type {
  AssistantMessageViewModel,
  AssistantSegmentViewModel,
  MessageViewModel,
  SystemInfoViewModel,
  SystemLevel,
  ToolResultViewModel,
  ToolUseViewModel,
  UserImageViewModel,
  UserTextViewModel,
} from '../view-model/types.js'
import { imageSourceToRef, normalizeToolResultContent } from './content-blocks.js'
import {
  describeToolInput,
  summarizeToolInput,
} from './tool-input.js'
import { classifyToolStatus } from './tool-status.js'

const KNOWN_SYSTEM_LEVELS: ReadonlySet<SystemLevel> = new Set<SystemLevel>([
  'info',
  'warning',
  'error',
  'success',
  'debug',
])

/**
 * Normalize a free-form level string (from `RawMessage.level`) into the
 * closed `SystemLevel` set. Unknown values fall back to `'info'`. Exported
 * so message leaf components can share the mapping without re-running
 * the full adapter.
 */
export function systemLevelFromRaw(level: string | undefined): SystemLevel {
  if (!level) {
    return 'info'
  }
  const lowered = level.toLowerCase() as SystemLevel
  return KNOWN_SYSTEM_LEVELS.has(lowered) ? lowered : 'info'
}

function segmentKey(raw: RawMessage, index: number): string {
  return `${raw.id}:assistant:${index}`
}

function userSegmentKey(raw: RawMessage, index: number): string {
  return `${raw.id}:user:${index}`
}

function userImageKey(raw: RawMessage, index: number): string {
  return `${raw.id}:image:${index}`
}

interface SegmentCollector {
  text: string[]
  thinking: string[]
  redacted: boolean
}

function flushAssistantSegment(
  segments: AssistantSegmentViewModel[],
  collector: SegmentCollector,
): void {
  const text = collector.text.join('\n').trim()
  const thinkingJoined = collector.thinking
    .map(piece => piece.trim())
    .filter(Boolean)
    .join('\n')
  const thinking = thinkingJoined || undefined

  if (!text && !thinking) {
    collector.text = []
    collector.thinking = []
    collector.redacted = false
    return
  }

  segments.push({
    index: segments.length,
    text,
    thinking,
    redactedThinking: thinking ? collector.redacted || undefined : undefined,
  })
  collector.text = []
  collector.thinking = []
  collector.redacted = false
}

/**
 * Convert one `RawMessage` (from the live store) into the view-model
 * items it represents. Most raw messages produce a single view model,
 * but user/assistant messages with rich `contentBlocks` may produce
 * several (e.g. text followed by an image).
 */
export function mapRawMessageToViewModels(
  raw: RawMessage,
): MessageViewModel[] {
  switch (raw.role) {
    case 'tool_use':
      return [mapToolUse(raw)]
    case 'tool_result':
      return [mapToolResult(raw)]
    case 'assistant':
      return mapAssistant(raw)
    case 'user':
      return mapUser(raw)
    case 'system':
      return mapSystem(raw)
    default:
      return []
  }
}

/** Map a `ConversationMessage` (from replay) into view-model items. */
export function mapConversationMessageToViewModels(
  message: ConversationMessage,
): MessageViewModel[] {
  const raw: RawMessage = {
    id: message.id,
    role: message.role,
    content: message.content,
    timestamp: message.timestamp,
    contentBlocks: message.content_blocks,
    costUsd: message.cost_usd,
    thinking: message.thinking,
    level: message.level,
  }
  return mapRawMessageToViewModels(raw)
}

function mapToolUse(raw: RawMessage): ToolUseViewModel {
  const input = raw.toolInput
  return {
    kind: 'tool_use',
    id: `tool:${raw.toolUseId ?? raw.id}`,
    toolUseId: raw.toolUseId ?? raw.id,
    name: raw.toolName ?? 'Unknown Tool',
    input,
    inputDetail: describeToolInput(input),
    inputSummary: summarizeToolInput(input),
    timestamp: raw.timestamp,
    status: 'pending',
  }
}

function mapToolResult(raw: RawMessage): ToolResultViewModel {
  const toolUseId = raw.toolUseId ?? raw.id
  const isError = raw.isError ?? false
  const status = classifyToolStatus(raw.content ?? '', isError)
  return {
    kind: 'tool_result',
    id: `tool-result:${toolUseId}:${raw.timestamp}`,
    toolUseId,
    content: { text: raw.content ?? '', images: [] },
    status,
    isError: isError || status === 'error',
    timestamp: raw.timestamp,
  }
}

function mapAssistant(raw: RawMessage): MessageViewModel[] {
  const segments: AssistantSegmentViewModel[] = []
  const collector: SegmentCollector = {
    text: [],
    thinking: [],
    redacted: false,
  }
  const extras: MessageViewModel[] = []

  if (raw.contentBlocks?.length) {
    for (const block of raw.contentBlocks) {
      switch (block.type) {
        case 'text':
          if (block.text) {
            collector.text.push(block.text)
          }
          break
        case 'thinking':
          if (block.thinking) {
            collector.thinking.push(block.thinking)
          }
          break
        case 'redacted_thinking':
          collector.thinking.push('[redacted thinking]')
          collector.redacted = true
          break
        case 'tool_use':
          flushAssistantSegment(segments, collector)
          extras.push({
            kind: 'tool_use',
            id: `tool:${block.id}`,
            toolUseId: block.id,
            name: block.name,
            input: block.input,
            inputDetail: describeToolInput(block.input),
            inputSummary: summarizeToolInput(block.input),
            timestamp: raw.timestamp,
            status: 'pending',
          })
          break
        case 'image':
          // assistants don't currently emit images through this path, but
          // if they do we surface each image as its own assistant segment
          // placeholder so the composer still sees ordered output.
          flushAssistantSegment(segments, collector)
          segments.push({ index: segments.length, text: '[image omitted]' })
          imageSourceToRef(block.source) // retain in case callers want ref
          break
        default:
          break
      }
    }
    flushAssistantSegment(segments, collector)
  } else {
    if (raw.content) {
      collector.text.push(raw.content)
    }
    if (raw.thinking) {
      collector.thinking.push(raw.thinking)
    }
    flushAssistantSegment(segments, collector)
  }

  const primary: AssistantMessageViewModel = {
    kind: 'assistant_message',
    id: raw.id,
    segments,
    timestamp: raw.timestamp,
    costUsd: raw.costUsd,
  }

  // Preserve deterministic ordering: assistant message (holding ordered
  // segments) followed by any extracted tool_use items, so higher layers
  // can interleave them with later tool_result view models by toolUseId.
  return segments.length > 0 || extras.length === 0
    ? [primary, ...extras]
    : extras
}

function mapUser(raw: RawMessage): MessageViewModel[] {
  const results: MessageViewModel[] = []

  if (raw.contentBlocks?.length) {
    const textParts: string[] = []
    let textIndex = 0
    const flushText = () => {
      const joined = textParts.join('\n').trim()
      if (joined) {
        const entry: UserTextViewModel = {
          kind: 'user_text',
          id: userSegmentKey(raw, textIndex++),
          text: joined,
          timestamp: raw.timestamp,
        }
        results.push(entry)
      }
      textParts.length = 0
    }

    for (const block of raw.contentBlocks) {
      switch (block.type) {
        case 'text':
          if (block.text) {
            textParts.push(block.text)
          }
          break
        case 'image': {
          flushText()
          const entry: UserImageViewModel = {
            kind: 'user_image',
            id: userImageKey(raw, results.length),
            image: imageSourceToRef(block.source),
            timestamp: raw.timestamp,
          }
          results.push(entry)
          break
        }
        case 'tool_result': {
          flushText()
          const content = normalizeToolResultContent(block.content)
          const isError = !!block.is_error
          const status = classifyToolStatus(content.text, isError)
          const entry: ToolResultViewModel = {
            kind: 'tool_result',
            id: `tool-result:${block.tool_use_id}:${raw.timestamp}`,
            toolUseId: block.tool_use_id,
            content,
            status,
            isError: isError || status === 'error',
            timestamp: raw.timestamp,
          }
          results.push(entry)
          break
        }
        default:
          break
      }
    }
    flushText()

    const hadRenderable = raw.contentBlocks.some(
      (block: FrontendContentBlock) =>
        block.type === 'text' ||
        block.type === 'image' ||
        block.type === 'tool_result',
    )
    if (!hadRenderable && raw.content) {
      const entry: UserTextViewModel = {
        kind: 'user_text',
        id: userSegmentKey(raw, 0),
        text: raw.content,
        timestamp: raw.timestamp,
      }
      results.push(entry)
    }
    return results
  }

  if (raw.content) {
    const entry: UserTextViewModel = {
      kind: 'user_text',
      id: userSegmentKey(raw, 0),
      text: raw.content,
      timestamp: raw.timestamp,
    }
    results.push(entry)
  }
  return results
}

function mapSystem(raw: RawMessage): SystemInfoViewModel[] {
  if (!raw.content) {
    return []
  }
  return [
    {
      kind: 'system_info',
      id: raw.id,
      text: raw.content,
      level: systemLevelFromRaw(raw.level),
      timestamp: raw.timestamp,
    },
  ]
}

// The `segmentKey` helper is exported for adapter-aware callers that want
// to derive stable IDs matching the existing render pipeline without
// re-implementing the format.
export const assistantSegmentId = segmentKey
