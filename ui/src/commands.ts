/** Slash command definitions for autocomplete (mirrors src/commands/mod.rs) */

export type CommandKind =
  | 'display'   // Show info only (help, version, cost, status, files, context...)
  | 'action'    // Execute something (clear, commit, compact, exit, copy, export...)
  | 'toggle'    // Flip a boolean (fast, brief)
  | 'select'    // Pick a value from predefined options (effort, model, branch...)
  | 'input'     // Requires free-form argument (login-code, sleep, model-add...)

export interface CommandDef {
  name: string
  aliases: string[]
  description: string
  kind: CommandKind
  /** Predefined options for 'select' kind commands */
  options?: string[]
  /** Hint for what argument is expected (for 'input' kind) */
  argHint?: string
}

export const COMMANDS: CommandDef[] = [
  // ── Display ──
  { name: 'help',             aliases: ['h', '?'],              description: 'Show available commands',          kind: 'display' },
  { name: 'version',          aliases: ['v'],                   description: 'Show current version',             kind: 'display' },
  { name: 'cost',             aliases: ['usage'],               description: 'Show token usage and cost',        kind: 'display' },
  { name: 'status',           aliases: [],                      description: 'Show session status',              kind: 'display' },
  { name: 'files',            aliases: [],                      description: 'List files in conversation',       kind: 'display' },
  { name: 'context',          aliases: ['ctx'],                 description: 'Show context usage information',   kind: 'display' },
  { name: 'extra-usage',      aliases: ['eu'],                  description: 'Extended token usage analysis',    kind: 'display' },
  { name: 'rate-limit-options', aliases: ['rlo', 'rate-limit'], description: 'Show rate limit info',             kind: 'display' },
  { name: 'session',          aliases: [],                      description: 'Show session info or list sessions', kind: 'display' },
  { name: 'skills',           aliases: [],                      description: 'List available skills',            kind: 'display' },
  { name: 'diff',             aliases: [],                      description: 'Show git diff of current changes', kind: 'display' },
  { name: 'mcp',              aliases: [],                      description: 'Open MCP server manager (subcommands: list, add, edit, …)', kind: 'display' },
  { name: 'agents-ui',        aliases: ['au'],                  description: 'Open the agents settings dialog', kind: 'action' },
  { name: 'assistant',        aliases: ['kairos'],              description: 'View assistant mode status',       kind: 'display' },
  { name: 'daemon',           aliases: [],                      description: 'View/control daemon process',      kind: 'display' },
  { name: 'channels',         aliases: [],                      description: 'View connected channels',          kind: 'display' },
  { name: 'permissions',      aliases: ['perms'],               description: 'View or modify tool permissions',  kind: 'display' },

  // ── Action ──
  { name: 'clear',            aliases: [],                      description: 'Clear conversation history',       kind: 'action' },
  { name: 'exit',             aliases: ['quit', 'q'],           description: 'Exit the REPL',                    kind: 'action' },
  { name: 'commit',           aliases: [],                      description: 'Create a git commit',              kind: 'action' },
  { name: 'compact',          aliases: [],                      description: 'Compact conversation to save tokens', kind: 'action' },
  { name: 'copy',             aliases: ['cp'],                  description: 'Copy last response to clipboard',  kind: 'action' },
  { name: 'export',           aliases: [],                      description: 'Export conversation to Markdown',   kind: 'action' },
  { name: 'audit-export',     aliases: ['audit'],               description: 'Export verifiable audit record',   kind: 'action' },
  { name: 'session-export',   aliases: ['sexport'],             description: 'Export structured JSON data',      kind: 'action' },
  { name: 'init',             aliases: [],                      description: 'Initialize project config',        kind: 'action' },
  { name: 'reload-plugins',   aliases: [],                      description: 'Hot-refresh the plugin registry',  kind: 'action' },
  { name: 'login',            aliases: [],                      description: 'Authenticate with Anthropic',      kind: 'action' },
  { name: 'logout',           aliases: [],                      description: 'Clear stored credentials',         kind: 'action' },
  { name: 'resume',           aliases: [],                      description: 'Resume a previous session',        kind: 'action' },
  { name: 'dream',            aliases: [],                      description: 'Distill daily logs into memory',   kind: 'action' },
  { name: 'memory',           aliases: ['mem'],                 description: 'View/manage project instructions', kind: 'action' },

  // ── Toggle ──
  { name: 'fast',             aliases: [],                      description: 'Toggle fast mode',                 kind: 'toggle' },
  { name: 'brief',            aliases: [],                      description: 'Toggle Brief output mode',         kind: 'toggle' },

  // ── Select ──
  { name: 'effort',           aliases: [],                      description: 'Set thinking effort level',        kind: 'select', options: ['low', 'medium', 'high'] },
  { name: 'model',            aliases: [],                      description: 'Show or switch the active model',  kind: 'select', options: ['sonnet', 'opus', 'haiku'] },
  { name: 'branch',           aliases: ['br'],                  description: 'Show or switch git branches',      kind: 'select' },
  { name: 'config',           aliases: ['settings'],            description: 'Show or modify configuration',     kind: 'select' },
  { name: 'notify',           aliases: [],                      description: 'Push notification settings',       kind: 'select', options: ['on', 'off'] },

  // ── Input (free-form arg) ──
  { name: 'login-code',       aliases: [],                      description: 'Complete OAuth login with code',   kind: 'input', argHint: '<code>' },
  { name: 'model-add',        aliases: ['ma'],                  description: 'Add a model with pricing to .env', kind: 'input', argHint: '<name> <input_price> <output_price>' },
  { name: 'sleep',            aliases: [],                      description: 'Set proactive sleep duration',     kind: 'input', argHint: '<duration>' },
  { name: 'add-dir',          aliases: [],                      description: 'Add a new working directory',      kind: 'input', argHint: '<path>' },
]

/** Match commands against a partial input (without leading /) */
export function matchCommands(partial: string): CommandDef[] {
  const q = partial.toLowerCase()
  if (!q) return [...COMMANDS]

  const results: Array<{ cmd: CommandDef; score: number }> = []
  for (const cmd of COMMANDS) {
    if (cmd.name.startsWith(q)) {
      results.push({ cmd, score: q === cmd.name ? 0 : 1 })
      continue
    }
    if (cmd.aliases.some(a => a.startsWith(q))) {
      results.push({ cmd, score: 2 })
      continue
    }
    if (cmd.name.includes(q)) {
      results.push({ cmd, score: 3 })
      continue
    }
    if (cmd.description.toLowerCase().includes(q)) {
      results.push({ cmd, score: 4 })
    }
  }

  results.sort((a, b) => a.score - b.score || a.cmd.name.localeCompare(b.cmd.name))
  return results.map(r => r.cmd)
}

/** Find exact command by name */
export function findCommand(name: string): CommandDef | undefined {
  const q = name.toLowerCase()
  return COMMANDS.find(c => c.name === q || c.aliases.includes(q))
}

/** Kind colors for display */
export const KIND_COLORS: Record<CommandKind, string> = {
  display: '#89B4FA',
  action:  '#F9E2AF',
  toggle:  '#A6E3A1',
  select:  '#CBA6F7',
  input:   '#F38BA8',
}
