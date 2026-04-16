import { useState, useMemo } from 'react'
import { executeCommand } from '@/lib/api'
import type { CommandInfo } from '@/lib/types'
import { Terminal, Search, Loader2, ChevronRight } from 'lucide-react'

interface CommandPaletteProps {
  commands?: CommandInfo[]
}

export function CommandPalette({ commands }: CommandPaletteProps) {
  const [search, setSearch] = useState('')
  const [executing, setExecuting] = useState<string | null>(null)
  const [expanded, setExpanded] = useState(false)

  const filtered = useMemo(() => {
    if (!commands) return []
    const q = search.toLowerCase()
    return commands.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        c.description.toLowerCase().includes(q) ||
        c.aliases.some((a) => a.toLowerCase().includes(q))
    )
  }, [commands, search])

  const handleExecute = async (cmd: CommandInfo) => {
    setExecuting(cmd.name)
    try {
      await executeCommand(cmd.name)
    } finally {
      setExecuting(null)
    }
  }

  const displayCommands = expanded ? filtered : filtered.slice(0, 8)

  return (
    <div className="space-y-2">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 text-xs font-medium text-muted-foreground uppercase tracking-wide hover:text-foreground transition-colors w-full"
      >
        <Terminal className="h-3 w-3" />
        Commands
        <span className="text-[10px] font-normal normal-case">
          ({commands?.length || 0})
        </span>
        <ChevronRight
          className={`h-3 w-3 ml-auto transition-transform ${expanded ? 'rotate-90' : ''}`}
        />
      </button>

      {expanded && (
        <>
          {/* Search */}
          <div className="relative">
            <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search commands..."
              className="w-full rounded border border-border bg-muted/50 pl-7 pr-2 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
          </div>

          {/* Command list */}
          <div className="max-h-64 overflow-y-auto space-y-0.5">
            {displayCommands.map((cmd) => (
              <button
                key={cmd.name}
                onClick={() => handleExecute(cmd)}
                disabled={executing === cmd.name}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs hover:bg-muted transition-colors group"
              >
                {executing === cmd.name ? (
                  <Loader2 className="h-3 w-3 animate-spin text-primary shrink-0" />
                ) : (
                  <span className="text-primary font-mono shrink-0">/</span>
                )}
                <span className="font-mono font-medium text-foreground/80 shrink-0">
                  {cmd.name}
                </span>
                <span className="truncate text-muted-foreground text-[10px]">
                  {cmd.description}
                </span>
              </button>
            ))}

            {filtered.length === 0 && (
              <div className="text-xs text-muted-foreground italic py-2 text-center">
                No matching commands
              </div>
            )}
          </div>
        </>
      )}
    </div>
  )
}
