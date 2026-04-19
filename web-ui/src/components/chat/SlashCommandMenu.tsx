import { useMemo, useEffect, useRef } from 'react'
import type { CommandInfo } from '@/lib/types'
import { Terminal } from 'lucide-react'

interface SlashCommandMenuProps {
  query: string
  commands: CommandInfo[]
  activeIndex: number
  onPick: (cmd: CommandInfo) => void
  onHover: (idx: number) => void
}

/**
 * Popover that appears above the input when the user is typing a slash
 * command. Does the fuzzy-ish filtering in-memory against the live command
 * list published on AppState.
 */
export function SlashCommandMenu({
  query,
  commands,
  activeIndex,
  onPick,
  onHover,
}: SlashCommandMenuProps) {
  const listRef = useRef<HTMLDivElement>(null)

  const filtered = useMemo(() => filterCommands(commands, query), [commands, query])

  // Keep the active entry in view as the user arrows through the list.
  useEffect(() => {
    if (!listRef.current) return
    const active = listRef.current.querySelector<HTMLElement>('[data-active="true"]')
    active?.scrollIntoView({ block: 'nearest' })
  }, [activeIndex])

  if (filtered.length === 0) {
    return (
      <div className="absolute bottom-full left-0 right-0 mx-2 mb-1 rounded-lg border border-border bg-popover shadow-lg overflow-hidden">
        <div className="px-3 py-2 text-xs text-muted-foreground italic">
          No commands match &ldquo;/{query}&rdquo;
        </div>
      </div>
    )
  }

  return (
    <div className="absolute bottom-full left-0 right-0 mx-2 mb-1 rounded-lg border border-border bg-popover shadow-lg overflow-hidden">
      <div className="flex items-center gap-2 border-b border-border/50 px-3 py-1.5 text-[10px] uppercase tracking-wide text-muted-foreground">
        <Terminal className="h-3 w-3" />
        <span>Commands</span>
        <span className="ml-auto">↑↓ Tab/Enter</span>
      </div>
      <div ref={listRef} className="max-h-64 overflow-y-auto py-1">
        {filtered.map((cmd, i) => {
          const isActive = i === activeIndex
          return (
            <button
              key={cmd.name}
              type="button"
              data-active={isActive}
              onMouseEnter={() => onHover(i)}
              onMouseDown={(e) => {
                e.preventDefault()
                onPick(cmd)
              }}
              className={`flex w-full items-baseline gap-2 px-3 py-1.5 text-left text-xs transition-colors ${
                isActive ? 'bg-primary/15 text-foreground' : 'hover:bg-muted'
              }`}
            >
              <span
                className={`shrink-0 font-mono ${
                  isActive ? 'text-primary' : 'text-muted-foreground'
                }`}
              >
                /
              </span>
              <span className="shrink-0 font-mono font-medium text-foreground">
                {cmd.name}
              </span>
              {cmd.aliases.length > 0 && (
                <span className="shrink-0 text-[10px] text-muted-foreground">
                  ({cmd.aliases.join(', ')})
                </span>
              )}
              <span className="truncate text-[11px] text-muted-foreground">
                {cmd.description}
              </span>
            </button>
          )
        })}
      </div>
    </div>
  )
}

/**
 * Find commands whose name / alias starts with the query, then those that
 * merely contain the query, then those whose description matches. Keeps
 * the list deterministic so arrow-key navigation is predictable.
 */
export function filterCommands(commands: CommandInfo[], query: string): CommandInfo[] {
  if (!commands || commands.length === 0) return []
  const q = query.toLowerCase().trim()
  if (!q) return commands.slice(0, 30)

  const starts: CommandInfo[] = []
  const contains: CommandInfo[] = []
  const descHit: CommandInfo[] = []
  const seen = new Set<string>()

  for (const c of commands) {
    const name = c.name.toLowerCase()
    const aliases = c.aliases.map((a) => a.toLowerCase())
    if (name.startsWith(q) || aliases.some((a) => a.startsWith(q))) {
      if (!seen.has(c.name)) {
        starts.push(c)
        seen.add(c.name)
      }
    } else if (name.includes(q) || aliases.some((a) => a.includes(q))) {
      if (!seen.has(c.name)) {
        contains.push(c)
        seen.add(c.name)
      }
    } else if (c.description.toLowerCase().includes(q)) {
      if (!seen.has(c.name)) {
        descHit.push(c)
        seen.add(c.name)
      }
    }
  }

  return [...starts, ...contains, ...descHit].slice(0, 30)
}
