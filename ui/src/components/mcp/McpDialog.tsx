import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useBackend } from '../../ipc/context.js'
import { useAppDispatch, useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'
import { MCPListPanel } from './MCPListPanel.js'
import { MCPRemoteServerMenu } from './MCPRemoteServerMenu.js'
import { MCPStdioServerMenu } from './MCPStdioServerMenu.js'
import { MCPToolDetailView } from './MCPToolDetailView.js'
import { MCPToolListView } from './MCPToolListView.js'
import type { MCPViewState } from './types.js'
import { buildServerInfos } from './utils.js'

/**
 * Top-level modal for `/mcp`. Matches the upstream `MCPSettings`
 * mode-machine: list → server-menu → server-tools → server-tool-detail.
 *
 * State comes from the `mcpSettings` slice (populated by
 * `config_list`, `server_list`, `tools_discovered`, etc. IPC events
 * the App dispatches). The dialog issues `query_config`,
 * `query_status`, and per-server `query_status` pings on mount so the
 * user always sees fresh data.
 */

export function McpDialog() {
  const { mcpSettings } = useAppState()
  const dispatch = useAppDispatch()
  const backend = useBackend()

  const [view, setView] = useState<MCPViewState>({ type: 'list' })
  const [listCursor, setListCursor] = useState(0)

  // Fetch fresh data on open.
  useEffect(() => {
    if (!mcpSettings.open) return
    setView({ type: 'list' })
    setListCursor(0)
    backend.send({ type: 'mcp_command', command: { kind: 'query_config' } })
    backend.send({ type: 'mcp_command', command: { kind: 'query_status' } })
  }, [mcpSettings.open, backend])

  const servers = useMemo(
    () =>
      buildServerInfos({
        entries: mcpSettings.entries,
        status: mcpSettings.status,
        toolsByServer: mcpSettings.toolsByServer,
        resourcesByServer: mcpSettings.resourcesByServer,
      }),
    [
      mcpSettings.entries,
      mcpSettings.status,
      mcpSettings.toolsByServer,
      mcpSettings.resourcesByServer,
    ],
  )

  // Keep the active `view.server` in sync with the latest `servers`
  // list — rebuilding `ServerInfo` on every entries/status change
  // otherwise leaves the menu staring at a stale reference.
  const viewMemo = useMemo<MCPViewState>(() => {
    if (view.type === 'list') return view
    const byName = new Map(servers.map(s => [s.name, s]))
    if (view.type === 'server-menu') {
      const latest = byName.get(view.server.name)
      return latest ? { type: 'server-menu', server: latest } : { type: 'list' }
    }
    if (view.type === 'server-tools') {
      const latest = byName.get(view.server.name)
      return latest ? { type: 'server-tools', server: latest } : { type: 'list' }
    }
    const latest = byName.get(view.server.name)
    return latest
      ? { type: 'server-tool-detail', server: latest, toolIndex: view.toolIndex }
      : { type: 'list' }
  }, [view, servers])

  const close = useCallback(() => {
    dispatch({ type: 'MCP_SETTINGS_CLOSE' })
    dispatch({ type: 'MCP_SETTINGS_CLEAR_NOTICE' })
  }, [dispatch])

  if (!mcpSettings.open) return null

  const contents = (() => {
    switch (viewMemo.type) {
      case 'list':
        return (
          <MCPListPanel
            servers={servers}
            selectedIndex={listCursor}
            onHover={setListCursor}
            onSelect={server => setView({ type: 'server-menu', server })}
            onCancel={close}
            lastError={mcpSettings.lastError}
            lastMessage={mcpSettings.lastMessage}
          />
        )
      case 'server-menu': {
        const tools = mcpSettings.toolsByServer[viewMemo.server.name] ?? []
        const statusToolsCount = viewMemo.server.status?.tools_count ?? tools.length
        const transport = viewMemo.server.transport.toLowerCase()
        const isRemote = transport === 'sse' || transport === 'streamable-http' || transport === 'http'
        if (isRemote) {
          return (
            <MCPRemoteServerMenu
              server={viewMemo.server}
              serverToolsCount={statusToolsCount}
              onViewTools={() =>
                setView({ type: 'server-tools', server: viewMemo.server })
              }
              onCancel={() => setView({ type: 'list' })}
            />
          )
        }
        return (
          <MCPStdioServerMenu
            server={viewMemo.server}
            serverToolsCount={statusToolsCount}
            onViewTools={() =>
              setView({ type: 'server-tools', server: viewMemo.server })
            }
            onCancel={() => setView({ type: 'list' })}
          />
        )
      }
      case 'server-tools': {
        const tools = mcpSettings.toolsByServer[viewMemo.server.name] ?? []
        return (
          <MCPToolListView
            server={viewMemo.server}
            tools={tools}
            onSelect={(_tool, index) =>
              setView({
                type: 'server-tool-detail',
                server: viewMemo.server,
                toolIndex: index,
              })
            }
            onBack={() => setView({ type: 'server-menu', server: viewMemo.server })}
          />
        )
      }
      case 'server-tool-detail': {
        const tools = mcpSettings.toolsByServer[viewMemo.server.name] ?? []
        const tool = tools[viewMemo.toolIndex]
        if (!tool) {
          // Tools list changed out from under us — bounce back.
          setView({ type: 'server-tools', server: viewMemo.server })
          return null
        }
        return (
          <MCPToolDetailView
            tool={tool}
            server={viewMemo.server}
            onBack={() =>
              setView({ type: 'server-tools', server: viewMemo.server })
            }
          />
        )
      }
    }
  })()

  return (
    <box
      position="absolute"
      top={2}
      left={2}
      right={2}
      bottom={3}
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor={c.accent}
      backgroundColor={c.bg}
      paddingX={1}
      paddingY={1}
      title="MCP"
      titleAlignment="center"
    >
      {contents}
    </box>
  )
}
