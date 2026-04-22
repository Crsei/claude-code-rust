/** Barrel for the Lite adapter layer. See individual modules for details. */
export {
  imageSourceToRef,
  toolResultImageToRef,
  inlineBlocksFromContent,
  normalizeToolResultContent,
  normalizeToolResultBlocks,
} from './content-blocks.js'
export {
  extractFileEditContext,
  type FileEditContext,
  type FileEditEntry,
} from './file-edit.js'
export {
  assistantSegmentId,
  mapConversationMessageToViewModels,
  mapRawMessageToViewModels,
  systemLevelFromRaw,
} from './messages.js'
export {
  categorizePermissionTool,
  mapPermissionRequestToViewModel,
  parsePermissionOption,
} from './permissions.js'
export { describeToolInput, summarizeToolInput } from './tool-input.js'
export { classifyToolStatus, mergeToolStatuses } from './tool-status.js'
