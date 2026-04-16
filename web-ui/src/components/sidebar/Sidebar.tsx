import { useChatStore } from '@/lib/store'
import { ModelSelector } from './ModelSelector'
import { PermissionPanel } from './PermissionPanel'
import { UsagePanel } from './UsagePanel'
import { CommandPalette } from './CommandPalette'
import { Settings, X } from 'lucide-react'

interface SidebarProps {
  open: boolean
  onClose: () => void
}

/**
 * Settings sidebar — contains model selector, permissions, usage, and commands.
 */
export function Sidebar({ open, onClose }: SidebarProps) {
  const appState = useChatStore((s) => s.appState)

  if (!open) return null

  return (
    <aside className="w-80 border-l border-border flex flex-col overflow-hidden bg-background shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <div className="flex items-center gap-2">
          <Settings className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-medium">Settings</span>
        </div>
        <button
          onClick={onClose}
          className="rounded p-1 hover:bg-muted transition-colors"
          title="Close sidebar"
        >
          <X className="h-4 w-4 text-muted-foreground" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {appState ? (
          <>
            <ModelSelector model={appState.model} />
            <PermissionPanel
              mode={appState.permission_mode}
              thinking={appState.thinking_enabled}
              fastMode={appState.fast_mode}
              effort={appState.effort}
            />
            <UsagePanel usage={appState.usage} />
            <CommandPalette commands={appState.commands} />
          </>
        ) : (
          <div className="text-sm text-muted-foreground italic text-center py-8">
            Connecting to server...
          </div>
        )}
      </div>
    </aside>
  )
}
