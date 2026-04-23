import type { McpServerStatusInfo } from '../../ipc/protocol.js'

/**
 * Small helper to shape the user-facing message after a reconnect
 * request. Mirrors the upstream `handleReconnectResult` /
 * `handleReconnectError` pair but works against cc-rust's
 * `McpServerStatusInfo` (no `client.type` discriminator).
 */

export interface ReconnectOutcome {
  message: string
  success: boolean
}

export function describeReconnectResult(
  status: McpServerStatusInfo | undefined,
  serverName: string,
): ReconnectOutcome {
  if (!status) {
    return {
      message: `Queued reconnect for ${serverName}. Waiting for the session to pick it up.`,
      success: true,
    }
  }
  switch (status.state) {
    case 'connected':
      return { message: `Reconnected to ${serverName}.`, success: true }
    case 'pending':
    case 'connecting':
      return {
        message: `Reconnect queued for ${serverName}; status is still ${status.state}.`,
        success: true,
      }
    case 'disabled':
      return {
        message: `${serverName} is disabled — enable it first to reconnect.`,
        success: false,
      }
    case 'failed':
    case 'error':
      return {
        message: `Failed to reconnect to ${serverName}${
          status.error ? `: ${status.error}` : '.'
        }`,
        success: false,
      }
    default:
      return {
        message: `${serverName} reported state: ${status.state}`,
        success: false,
      }
  }
}

export function describeReconnectError(
  error: unknown,
  serverName: string,
): string {
  const msg = error instanceof Error ? error.message : String(error)
  return `Error reconnecting to ${serverName}: ${msg}`
}
