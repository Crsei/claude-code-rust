/**
 * Helpers for normalizing `FrontendContentBlock[]` and `ToolResultContent`
 * shapes into the view-model inline blocks.
 *
 * Keep these functions pure. Component code should call them rather than
 * walking protocol shapes by hand.
 */

import type {
  FrontendContentBlock,
  ImageSource,
  ToolResultContent,
  ToolResultContentInfo,
} from '../ipc/protocol.js'
import type {
  ImageRef,
  NormalizedImageBlock,
  NormalizedInlineBlock,
  NormalizedTextBlock,
  NormalizedThinkingBlock,
  NormalizedToolResultContent,
} from '../view-model/types.js'

export function imageSourceToRef(source: ImageSource): ImageRef {
  return { data: source.data, mediaType: source.media_type }
}

export function toolResultImageToRef(
  block: Extract<ToolResultContentInfo, { type: 'image' }>,
): ImageRef | undefined {
  if (!block.data) {
    return undefined
  }
  return {
    data: block.data,
    mediaType: block.media_type,
    sizeBytes: block.size_bytes,
  }
}

/**
 * Flatten an assistant/user `FrontendContentBlock[]` into the view-model's
 * inline block union. Tool use / tool result blocks are intentionally
 * excluded — they are promoted to standalone view-model items by the
 * message adapter.
 */
export function inlineBlocksFromContent(
  blocks: FrontendContentBlock[],
): NormalizedInlineBlock[] {
  const result: NormalizedInlineBlock[] = []

  for (const block of blocks) {
    switch (block.type) {
      case 'text': {
        if (block.text) {
          const entry: NormalizedTextBlock = { kind: 'text', text: block.text }
          result.push(entry)
        }
        break
      }
      case 'thinking': {
        if (block.thinking) {
          const entry: NormalizedThinkingBlock = {
            kind: 'thinking',
            text: block.thinking,
            redacted: false,
          }
          result.push(entry)
        }
        break
      }
      case 'redacted_thinking': {
        const entry: NormalizedThinkingBlock = {
          kind: 'thinking',
          text: '[redacted thinking]',
          redacted: true,
        }
        result.push(entry)
        break
      }
      case 'image': {
        const entry: NormalizedImageBlock = {
          kind: 'image',
          image: imageSourceToRef(block.source),
        }
        result.push(entry)
        break
      }
      default:
        // tool_use and tool_result are handled by the message adapter at
        // a higher level — they are not inline content from this layer's
        // perspective.
        break
    }
  }

  return result
}

/**
 * Normalize `ToolResultContent` (string or nested block array from the
 * `tool_result` protocol block). Returns the collapsed text plus any
 * images we could recover from nested blocks.
 */
export function normalizeToolResultContent(
  content: ToolResultContent,
): NormalizedToolResultContent {
  if (typeof content === 'string') {
    return { text: content, images: [] }
  }

  const textParts: string[] = []
  const images: ImageRef[] = []

  for (const block of content) {
    switch (block.type) {
      case 'text':
        if (block.text) {
          textParts.push(block.text)
        }
        break
      case 'thinking':
        if (block.thinking) {
          textParts.push(block.thinking)
        }
        break
      case 'image':
        images.push(imageSourceToRef(block.source))
        break
      case 'tool_result': {
        const nested = normalizeToolResultContent(block.content)
        if (nested.text) {
          textParts.push(nested.text)
        }
        for (const image of nested.images) {
          images.push(image)
        }
        break
      }
      default:
        break
    }
  }

  return { text: textParts.join('\n'), images }
}

/**
 * Merge a `tool_result` backend message's plain `output` string with its
 * optional `content_blocks` into the normalized shape. Images from
 * `content_blocks` take precedence; the plain string provides the
 * authoritative text body.
 */
export function normalizeToolResultBlocks(
  output: string,
  blocks: ToolResultContentInfo[] | undefined,
): NormalizedToolResultContent {
  if (!blocks || blocks.length === 0) {
    return { text: output, images: [] }
  }

  const textParts: string[] = []
  const images: ImageRef[] = []

  for (const block of blocks) {
    if (block.type === 'text') {
      if (block.text) {
        textParts.push(block.text)
      }
      continue
    }
    if (block.type === 'image') {
      const ref = toolResultImageToRef(block)
      if (ref) {
        images.push(ref)
      }
    }
  }

  const blockText = textParts.join('\n').trim()
  const fallbackText = output?.trim() ?? ''
  const text = blockText || fallbackText

  return { text, images }
}
