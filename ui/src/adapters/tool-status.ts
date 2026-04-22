import type { ToolStatus } from '../view-model/types.js'

/**
 * Derive a `ToolStatus` from the raw `output` text and `is_error` flag that
 * the backend sends in a `tool_result` message. Mirrors the heuristic in
 * `ui/src/store/message-model.ts` so the adapter and the existing store
 * stay consistent.
 */
export function classifyToolStatus(
  output: string,
  isError: boolean,
): ToolStatus {
  const lowered = output.toLowerCase()
  if (
    lowered.includes('interrupted by user') ||
    lowered.includes('cancelled') ||
    lowered.includes('aborted')
  ) {
    return 'cancelled'
  }
  if (isError) {
    return 'error'
  }
  return 'success'
}

/** Collapse a set of child statuses into one aggregate status, for grouped
 * tool activities. Priority mirrors the store: error > running > pending
 * > cancelled > success. */
export function mergeToolStatuses(statuses: ToolStatus[]): ToolStatus {
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
