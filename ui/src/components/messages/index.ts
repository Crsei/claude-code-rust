/**
 * Barrel for the OpenTUI message leaf components.
 *
 * Each leaf renders exactly one discriminant of the pipeline's
 * `RenderItem` shape (from `ui/src/store/message-model.ts`) and consumes
 * the Issue 01 adapter layer where normalization is useful
 * (`systemLevelFromRaw`, `ToolStatus`). The leaves are designed to be
 * composed by a thin dispatcher (`MessageBubble`) rather than by the
 * sample tree's monolithic `Message`/`MessageRow` runtime.
 */
export { AssistantTextMessage } from './AssistantTextMessage.js'
export { CompactBoundaryMessage } from './CompactBoundaryMessage.js'
export { FileEditToolPreview, isFileEditToolName } from './FileEditToolPreview.js'
export { StreamingMessage } from './StreamingMessage.js'
export { SystemMessage } from './SystemMessage.js'
export { ThinkingPreview } from './ThinkingPreview.js'
export { ToolActivityMessage } from './ToolActivityMessage.js'
export { ToolGroupMessage } from './ToolGroupMessage.js'
export { ToolResultOrphanMessage } from './ToolResultOrphanMessage.js'
export { UserTextMessage } from './UserTextMessage.js'

// ---------------------------------------------------------------------------
// Intentionally not ported from upstream — require IPC/state data that does
// not exist in cc-rust today. Reintroduce only with a matching backend signal:
//
// - RateLimitMessage / SystemAPIErrorMessage — rate-limit/API errors arrive
//   as plain system_info text; upgrade once the backend sends structured
//   error events with code/retry-after.
// - TaskAssignmentMessage / UserTeammateMessage / UserChannelMessage /
//   UserAgentNotificationMessage — agent-swarm / KAIROS channel UI; cc-rust
//   daemon surfaces these but the frontend IPC does not yet expose them.
// - UserImageMessage / AttachmentMessage — depend on the ImageSource
//   refs the backend does not yet forward.
// - PlanApprovalMessage / UserPlanMessage / UserResourceUpdateMessage /
//   UserLocalCommandOutputMessage / UserBashOutputMessage — need tagged
//   content (`<bash-stdout>`, `<plan>`, `<mcp-resource-update>`) which
//   cc-rust strips before IPC.
// - ShutdownMessage / HookProgressMessage — no hook/shutdown event stream
//   in the current IPC protocol.
// - AdvisorMessage / AssistantRedactedThinkingMessage — niche upstream
//   cases; our redacted-thinking renders via ThinkingPreview already.
// ---------------------------------------------------------------------------
