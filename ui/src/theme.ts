export const theme = {
  // Roles
  assistantName: { color: 'ansi:magenta', bold: true },
  userName: { color: 'ansi:cyanBright', bold: true },
  systemName: { color: 'ansi:white', dim: true },
  toolName: { color: 'ansi:yellow', bold: true },

  // Severity
  error: { color: 'ansi:red', bold: true },
  warning: { color: 'ansi:yellow' },
  info: { color: 'ansi:cyan' },

  // Code
  code: { color: 'ansi:yellow' },

  // Diff
  diffAdd: { color: 'ansi:green' },
  diffRemove: { color: 'ansi:red' },
  diffMeta: { color: 'ansi:cyan', dim: true },

  // UI
  border: { color: 'ansi:white', dim: true },
  accent: { color: 'ansi:magenta' },
  muted: { dim: true },
} as const
