/**
 * Browser MCP classification — mirrors `src/browser/detection.rs` +
 * `src/browser/permissions.rs` on the Rust side. Kept small and self-contained
 * so the Web UI can stamp tool cards with a category label without a round
 * trip to the backend.
 */

export type BrowserCategory =
  | 'navigation'
  | 'read'
  | 'write'
  | 'upload'
  | 'javascript'
  | 'observability'
  | 'other'

const BROWSER_TOOL_BASENAMES = new Set<string>([
  // Navigation / tabs
  'navigate', 'navigate_page', 'goto',
  'tabs_create', 'tabs_create_mcp', 'tabs_close', 'tabs_close_mcp',
  'tabs_context', 'tabs_context_mcp',
  'new_page', 'close_page', 'switch_browser', 'select_page', 'list_pages',
  // Page reading
  'read_page', 'get_page_text', 'take_snapshot', 'snapshot', 'get_page',
  // DOM / element interaction
  'click', 'browser_click', 'double_click', 'hover', 'drag',
  'press_key', 'type_text', 'fill', 'fill_form', 'form_input', 'select',
  // File upload
  'upload_file', 'file_upload',
  // JS execution
  'evaluate_script', 'javascript_tool', 'evaluate',
  // Console / network observability
  'get_console_message', 'list_console_messages', 'read_console_messages',
  'get_network_request', 'list_network_requests', 'read_network_requests',
  // Screenshots / visual
  'take_screenshot', 'screenshot',
  // Misc
  'wait_for', 'find', 'resize_page', 'resize_window', 'emulate', 'handle_dialog',
])

/** Parse `mcp__{server}__{action}` into `[server, action]` or null. */
function splitMcpName(toolName: string): [string, string] | null {
  if (!toolName.startsWith('mcp__')) return null
  const rest = toolName.slice('mcp__'.length)
  const idx = rest.indexOf('__')
  if (idx < 0) return null
  return [rest.slice(0, idx), rest.slice(idx + 2)]
}

/** Is this tool name a browser MCP tool by action basename? */
export function isBrowserTool(toolName: string | undefined | null): boolean {
  if (!toolName) return false
  const parts = splitMcpName(toolName)
  if (!parts) return false
  return BROWSER_TOOL_BASENAMES.has(parts[1])
}

/** Classify a browser tool name into a coarse-grained category. */
export function classifyBrowserTool(toolName: string | undefined | null): BrowserCategory | null {
  if (!toolName) return null
  const parts = splitMcpName(toolName)
  if (!parts) return null
  return classifyBrowserAction(parts[1])
}

/** Classify an action basename (without the `mcp__{server}__` prefix). */
export function classifyBrowserAction(action: string): BrowserCategory {
  switch (action) {
    case 'navigate': case 'navigate_page': case 'goto':
    case 'tabs_create': case 'tabs_create_mcp': case 'tabs_close':
    case 'tabs_close_mcp': case 'tabs_context': case 'tabs_context_mcp':
    case 'new_page': case 'close_page': case 'switch_browser':
    case 'select_page': case 'list_pages':
      return 'navigation'
    case 'read_page': case 'get_page_text': case 'take_snapshot':
    case 'snapshot': case 'get_page': case 'take_screenshot':
    case 'screenshot': case 'find': case 'wait_for':
      return 'read'
    case 'click': case 'browser_click': case 'double_click': case 'hover':
    case 'drag': case 'press_key': case 'type_text': case 'fill':
    case 'fill_form': case 'form_input': case 'select':
    case 'resize_page': case 'resize_window': case 'emulate':
    case 'handle_dialog':
      return 'write'
    case 'upload_file': case 'file_upload':
      return 'upload'
    case 'evaluate_script': case 'javascript_tool': case 'evaluate':
      return 'javascript'
    case 'get_console_message': case 'list_console_messages':
    case 'read_console_messages': case 'get_network_request':
    case 'list_network_requests': case 'read_network_requests':
      return 'observability'
    default:
      return 'other'
  }
}

/** Color class for a category label. Intentionally subtle — Tailwind tokens. */
export function categoryColorClass(cat: BrowserCategory): string {
  switch (cat) {
    case 'navigation': return 'text-sky-400 border-sky-400/40'
    case 'read':        return 'text-emerald-400 border-emerald-400/40'
    case 'write':       return 'text-amber-400 border-amber-400/40'
    case 'upload':      return 'text-fuchsia-400 border-fuchsia-400/40'
    case 'javascript':  return 'text-rose-400 border-rose-400/40'
    case 'observability': return 'text-violet-400 border-violet-400/40'
    case 'other':
    default:            return 'text-muted-foreground border-border/40'
  }
}
