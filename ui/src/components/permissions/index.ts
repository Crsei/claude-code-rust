/**
 * Barrel for the Lite-native permission UI family (Issue 04).
 *
 * Consumers should import `PermissionRequestDialog` — a category-aware
 * dialog that runs the incoming `PermissionRequest` through the Issue
 * 01 adapter and picks a body variant (`BashPermissionRequest`,
 * `FileEditPermissionRequest`, `FileWritePermissionRequest`,
 * `WebFetchPermissionRequest`, `FallbackPermissionRequest`) from this
 * module.
 *
 * Individual variants are exported too so higher-level features
 * (e.g. an inline planning approval prompt) can reuse them without
 * going through the dialog frame.
 */
export { BashPermissionRequest } from './BashPermissionRequest.js'
export { FallbackPermissionRequest } from './FallbackPermissionRequest.js'
export { FileEditPermissionRequest } from './FileEditPermissionRequest.js'
export { FileWritePermissionRequest } from './FileWritePermissionRequest.js'
export { PermissionDialogFrame } from './PermissionDialogFrame.js'
export {
  PermissionPromptOptions,
  resolveHotkey,
} from './PermissionPromptOptions.js'
export { PermissionRequestDialog } from './PermissionRequestDialog.js'
export { WebFetchPermissionRequest } from './WebFetchPermissionRequest.js'
