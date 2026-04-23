/**
 * Barrel for shell output UI components.
 *
 * Mirrors the upstream `ui/examples/upstream-patterns/src/components/shell/`
 * layout:
 *   - `ExpandShellOutputContext` — context flag for full vs truncated output
 *   - `OutputLine`               — JSON-aware, ANSI-stripped output renderer
 *   - `ShellProgressMessage`     — running/completed shell tail view
 *   - `ShellTimeDisplay`         — elapsed/timeout annotation
 */

export {
  ExpandShellOutputProvider,
  useExpandShellOutput,
} from './ExpandShellOutputContext.js'
export { OutputLine, tryFormatJson, tryJsonFormatContent } from './OutputLine.js'
export { ShellProgressMessage } from './ShellProgressMessage.js'
export { ShellTimeDisplay } from './ShellTimeDisplay.js'
export { formatDuration, formatFileSize, stripAnsi } from './format.js'
