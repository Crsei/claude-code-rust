/**
 * Normalized view-model types shared by the active OpenTUI Lite frontend and
 * migration slices imported from `ui/examples/upstream-patterns/`.
 *
 * The view-model sits between the raw IPC protocol (`ui/src/ipc/protocol.ts`)
 * plus the in-store `RawMessage` shape (`ui/src/store/message-model.ts`) and
 * the component layer. Components that migrate from the sample tree should
 * consume these types instead of touching `FrontendContentBlock`,
 * `ConversationMessage`, or `PermissionRequest` directly.
 *
 * No runtime dependency on the sample tree. Keep this file pure types so it
 * stays cheap to import from any adapter or component.
 */

export type ToolStatus =
  | 'pending'
  | 'running'
  | 'success'
  | 'error'
  | 'cancelled'

/** Base64 image payload shared by assistant/user content blocks and tool
 * results. Mirrors the fields the Rust backend forwards over IPC. */
export interface ImageRef {
  /** Base64 data without a `data:` prefix. */
  data: string
  /** MIME media type (e.g. `image/png`). */
  mediaType: string
  /** Decoded image size in bytes, when the backend knows it. */
  sizeBytes?: number
}

export interface NormalizedTextBlock {
  kind: 'text'
  text: string
}

export interface NormalizedThinkingBlock {
  kind: 'thinking'
  text: string
  /** Present when the source block was `redacted_thinking`. */
  redacted: boolean
}

export interface NormalizedImageBlock {
  kind: 'image'
  image: ImageRef
}

export type NormalizedInlineBlock =
  | NormalizedTextBlock
  | NormalizedThinkingBlock
  | NormalizedImageBlock

/** A single assistant-turn segment, split on tool_use boundaries so each
 * chunk is renderable as one bubble. */
export interface AssistantSegmentViewModel {
  /** Zero-based position within the parent assistant message. */
  index: number
  text: string
  thinking?: string
  /** `true` when any portion of `thinking` came from a redacted block. */
  redactedThinking?: boolean
}

export interface UserTextViewModel {
  kind: 'user_text'
  id: string
  text: string
  timestamp: number
}

export interface UserImageViewModel {
  kind: 'user_image'
  id: string
  image: ImageRef
  timestamp: number
}

export interface AssistantMessageViewModel {
  kind: 'assistant_message'
  id: string
  segments: AssistantSegmentViewModel[]
  timestamp: number
  costUsd?: number
}

export interface ToolUseViewModel {
  kind: 'tool_use'
  id: string
  toolUseId: string
  name: string
  input: unknown
  /** Full human-readable rendering of the key input argument. */
  inputDetail: string
  /** One-line compacted version of `inputDetail`. */
  inputSummary: string
  timestamp: number
  status: ToolStatus
}

export interface NormalizedToolResultContent {
  /** Flattened text view. Empty string when the result was image-only. */
  text: string
  /** Image attachments extracted from the tool result (e.g. browser MCP
   * screenshots). */
  images: ImageRef[]
}

export interface ToolResultViewModel {
  kind: 'tool_result'
  id: string
  toolUseId: string
  content: NormalizedToolResultContent
  status: ToolStatus
  isError: boolean
  timestamp: number
}

export type SystemLevel =
  | 'info'
  | 'warning'
  | 'error'
  | 'success'
  | 'debug'

export interface SystemInfoViewModel {
  kind: 'system_info'
  id: string
  text: string
  level: SystemLevel
  timestamp: number
}

/**
 * Categorization for the permission request UI. Derived from the backend
 * tool name by the adapter so components can dispatch on a small closed set
 * instead of doing string matching themselves.
 */
export type PermissionCategory =
  | 'bash'
  | 'file_edit'
  | 'file_write'
  | 'web_fetch'
  | 'tool_generic'

export interface PermissionOption {
  /** Value sent back over IPC as `permission_response.decision`. */
  value: string
  /** Human-readable label derived from the backend option string. */
  label: string
  /** Hotkey letter when the backend provided one (e.g. `y`, `n`, `a`). */
  hotkey?: string
}

export interface PermissionRequestViewModel {
  kind: 'permission_request'
  toolUseId: string
  tool: string
  command: string
  options: PermissionOption[]
  category: PermissionCategory
}

/** Discriminated union covering every renderable message-layer view model. */
export type MessageViewModel =
  | UserTextViewModel
  | UserImageViewModel
  | AssistantMessageViewModel
  | ToolUseViewModel
  | ToolResultViewModel
  | SystemInfoViewModel
