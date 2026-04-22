/**
 * Placeholder barrel for the MCP operational-panel migration slice.
 *
 * New MCP components should subscribe to the current store's
 * `subsystems.mcp` snapshot and send `mcp_command` payloads through
 * `useBackend()` from `ui/src/ipc/context.tsx`. Do not import
 * directly from `ui/examples/upstream-patterns/`.
 *
 * No active exports yet — see Issue 05 (operational panels for MCP,
 * LSP, team, and shell).
 */
export {}
