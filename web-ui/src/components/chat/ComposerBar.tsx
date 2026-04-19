import { useState, useRef, useCallback, useMemo, useEffect } from 'react'
import { Send, Square, Paperclip, Shield, Zap, Brain } from 'lucide-react'
import { SlashCommandMenu, filterCommands } from './SlashCommandMenu'
import { executeCommand, updateSetting } from '@/lib/api'
import { useChatStore } from '@/lib/store'
import type { CommandInfo } from '@/lib/types'

interface ComposerBarProps {
  onSend: (text: string) => void
  onAbort: () => void
  isStreaming: boolean
}

const PERMISSION_MODES = [
  { value: 'default', label: 'Default' },
  { value: 'auto', label: 'Auto' },
  { value: 'plan', label: 'Plan' },
  { value: 'bypass', label: 'Bypass' },
] as const

/**
 * Enhanced input composer. Wraps the plain textarea with:
 *   - slash-command suggestions (activated when input begins with /)
 *   - file attachment button (prepends @path tokens)
 *   - permission-mode toggle chips
 *   - model / effort readout
 *   - abort + send controls
 */
export function ComposerBar({ onSend, onAbort, isStreaming }: ComposerBarProps) {
  const appState = useChatStore((s) => s.appState)
  const commands: CommandInfo[] = appState?.commands ?? []
  const model = appState?.model ?? 'unknown'
  const permissionMode = appState?.permission_mode ?? 'default'
  const effort = appState?.effort ?? null
  const fastMode = appState?.fast_mode ?? false
  const thinkingEnabled = appState?.thinking_enabled === true

  const [input, setInput] = useState('')
  const [activeCmdIdx, setActiveCmdIdx] = useState(0)
  const [permLoading, setPermLoading] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Slash-command state — only active when the *entire* input still looks
  // like a single `/command` token. Once the user types a space, we hand off
  // to normal send behavior and the menu hides itself.
  const slashState = useMemo(() => {
    const trimmed = input.trimStart()
    if (!trimmed.startsWith('/')) return null
    const rest = trimmed.slice(1)
    // If there's a space anywhere, the user has moved on to arguments.
    if (rest.includes(' ') || rest.includes('\n')) return null
    return { query: rest }
  }, [input])

  const filteredCmds = useMemo(
    () => (slashState ? filterCommands(commands, slashState.query) : []),
    [slashState, commands],
  )

  // Reset selection when filter changes.
  useEffect(() => {
    setActiveCmdIdx(0)
  }, [slashState?.query])

  const focusTextarea = useCallback(() => {
    textareaRef.current?.focus()
  }, [])

  const pickCommand = useCallback(
    async (cmd: CommandInfo) => {
      setInput('')
      // Reset textarea height
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto'
      }
      try {
        await executeCommand(cmd.name)
      } finally {
        focusTextarea()
      }
    },
    [focusTextarea],
  )

  const handleSubmit = useCallback(async () => {
    if (isStreaming) {
      onAbort()
      return
    }
    const text = input.trim()
    if (!text) return

    // If the entire input is a bare /command, dispatch it directly.
    if (text.startsWith('/')) {
      const firstSpace = text.indexOf(' ')
      const name = (firstSpace === -1 ? text.slice(1) : text.slice(1, firstSpace)).trim()
      const args = firstSpace === -1 ? '' : text.slice(firstSpace + 1).trim()
      const cmd = commands.find((c) => c.name === name || c.aliases.includes(name))
      if (cmd) {
        setInput('')
        if (textareaRef.current) textareaRef.current.style.height = 'auto'
        try {
          await executeCommand(cmd.name, args)
        } finally {
          focusTextarea()
        }
        return
      }
      // Unknown command — fall through and send as chat text.
    }

    onSend(text)
    setInput('')
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto'
    }
  }, [input, isStreaming, onSend, onAbort, commands, focusTextarea])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Slash-command navigation takes precedence over normal keys.
      if (slashState && filteredCmds.length > 0) {
        if (e.key === 'ArrowDown') {
          e.preventDefault()
          setActiveCmdIdx((i) => Math.min(i + 1, filteredCmds.length - 1))
          return
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault()
          setActiveCmdIdx((i) => Math.max(i - 1, 0))
          return
        }
        if (e.key === 'Tab') {
          e.preventDefault()
          const cmd = filteredCmds[activeCmdIdx]
          if (cmd) setInput(`/${cmd.name} `)
          return
        }
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault()
          const cmd = filteredCmds[activeCmdIdx]
          if (cmd) void pickCommand(cmd)
          return
        }
        if (e.key === 'Escape') {
          e.preventDefault()
          setInput('')
          return
        }
      }

      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        void handleSubmit()
      }
    },
    [slashState, filteredCmds, activeCmdIdx, handleSubmit, pickCommand],
  )

  const handleInput = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value)
    const ta = e.target
    ta.style.height = 'auto'
    ta.style.height = Math.min(ta.scrollHeight, 200) + 'px'
  }, [])

  const handleAttachFiles = useCallback((files: FileList | null) => {
    if (!files || files.length === 0) return
    // Web UI runs in a sandboxed browser — we can't read the absolute path.
    // Insert @-tokens using the file name as a hint; the user is expected to
    // adjust to the correct repo-relative path.
    const tokens = Array.from(files)
      .map((f) => {
        const rel = (f as File & { webkitRelativePath?: string }).webkitRelativePath || f.name
        return `@${rel}`
      })
      .join(' ')
    setInput((prev) => (prev ? `${prev} ${tokens}` : tokens))
    focusTextarea()
  }, [focusTextarea])

  const handlePermChange = useCallback(async (mode: string) => {
    if (permLoading || mode === permissionMode) return
    setPermLoading(true)
    try {
      await updateSetting('set_permission_mode', mode)
    } finally {
      setPermLoading(false)
    }
  }, [permLoading, permissionMode])

  const disableSend = !isStreaming && !input.trim()

  return (
    <div className="border-t border-border bg-background">
      {/* Slash-command menu floats above the input */}
      <div className="relative mx-auto max-w-3xl px-4">
        {slashState && (
          <SlashCommandMenu
            query={slashState.query}
            commands={commands}
            activeIndex={activeCmdIdx}
            onPick={pickCommand}
            onHover={setActiveCmdIdx}
          />
        )}
      </div>

      {/* Main input row */}
      <div className="mx-auto max-w-3xl px-4 pt-3">
        <div className="flex items-end gap-2">
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            className="h-10 w-10 shrink-0 rounded-xl border border-input bg-card text-muted-foreground hover:text-foreground hover:border-foreground/30 transition-colors flex items-center justify-center"
            title="Reference file(s) — inserts @path tokens the assistant can pick up"
          >
            <Paperclip className="h-4 w-4" />
          </button>
          <input
            ref={fileInputRef}
            type="file"
            multiple
            className="hidden"
            onChange={(e) => {
              handleAttachFiles(e.target.files)
              // allow re-selecting the same file next time
              e.target.value = ''
            }}
          />

          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            placeholder={
              isStreaming
                ? 'Generating… press Stop to cancel'
                : 'Type /command, a message, or reference files with @path…'
            }
            rows={1}
            className="flex-1 resize-none rounded-xl border border-input bg-card px-4 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
          />

          <button
            type="button"
            onClick={handleSubmit}
            disabled={disableSend}
            className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl transition-colors ${
              isStreaming
                ? 'bg-destructive text-destructive-foreground hover:bg-destructive/90'
                : 'bg-primary text-primary-foreground hover:bg-primary/90'
            } disabled:opacity-40`}
            title={isStreaming ? 'Stop generation' : 'Send (Enter)'}
          >
            {isStreaming ? <Square className="h-4 w-4" /> : <Send className="h-4 w-4" />}
          </button>
        </div>
      </div>

      {/* Footer row: permissions + model/effort */}
      <div className="mx-auto flex max-w-3xl flex-wrap items-center gap-x-3 gap-y-1 px-4 pb-3 pt-2 text-[11px]">
        {/* Permission chips */}
        <div className="flex items-center gap-1" title="Permission mode — applies to the next tool call">
          <Shield className="h-3 w-3 text-muted-foreground" />
          <div className="flex overflow-hidden rounded-md border border-border">
            {PERMISSION_MODES.map((pm) => (
              <button
                key={pm.value}
                type="button"
                onClick={() => void handlePermChange(pm.value)}
                disabled={permLoading}
                className={`px-2 py-0.5 text-[11px] transition-colors ${
                  permissionMode === pm.value
                    ? 'bg-primary/15 text-primary font-medium'
                    : 'text-muted-foreground hover:text-foreground hover:bg-muted'
                } disabled:opacity-60`}
              >
                {pm.label}
              </button>
            ))}
          </div>
        </div>

        {/* Model readout */}
        <div
          className="flex items-center gap-1 text-muted-foreground truncate max-w-[220px]"
          title={`Model: ${model}`}
        >
          <Brain className="h-3 w-3 shrink-0" />
          <span className="truncate font-mono">{shortModelName(model)}</span>
          {fastMode && (
            <span className="ml-1 flex items-center gap-0.5 rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] text-amber-400">
              <Zap className="h-2.5 w-2.5" />
              fast
            </span>
          )}
          {thinkingEnabled && (
            <span className="ml-1 rounded bg-sky-500/15 px-1.5 py-0.5 text-[10px] text-sky-400">
              thinking
            </span>
          )}
        </div>

        {/* Effort */}
        {effort && (
          <span
            className="rounded-md bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
            title={`Reasoning effort: ${effort}`}
          >
            effort: {effort}
          </span>
        )}

        {/* Hint */}
        <span className="ml-auto hidden sm:inline text-muted-foreground/60">
          Enter to send • Shift+Enter newline • / for commands
        </span>
      </div>
    </div>
  )
}

/** Trim long provider-qualified model names down to something readable. */
function shortModelName(model: string): string {
  if (!model) return 'unknown'
  // Strip obvious date suffixes: claude-sonnet-4-20250514 → claude-sonnet-4
  const noDate = model.replace(/-\d{8,}$/, '')
  // Strip provider prefixes: "openai/gpt-4o" → "gpt-4o"
  const parts = noDate.split('/')
  return parts[parts.length - 1]
}
