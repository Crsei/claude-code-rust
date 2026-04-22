import type {
  AgentDefinitionSource,
  AgentMemoryScope,
  AgentPermissionMode,
} from '../../../ipc/protocol.js'

/**
 * Accumulating state for the create-agent wizard. Each step mutates the
 * subset it owns and advances; the final `ConfirmStep` builds the
 * `AgentDefinitionEntry` from this bag and sends it to the backend.
 *
 * Mirrors upstream `AgentWizardData` in
 * `ui/examples/upstream-patterns/src/components/agents/new-agent-creation/types.ts`.
 */
export interface AgentWizardData {
  /** Where the agent should be saved (`user` / `project`). */
  location?: Extract<AgentDefinitionSource, { kind: 'user' | 'project' }>['kind']

  /** `'generate'` = AI-assisted, `'manual'` = fill out by hand. */
  method?: 'generate' | 'manual'

  /** Freeform description the user typed into the generate step. */
  generationPrompt?: string

  /** `true` while the generation request is in flight. */
  isGenerating?: boolean

  /**
   * `true` when the final agent originated from AI generation — the confirm
   * step uses this to show an "AI-generated" tag and skip the user-facing
   * identifier-editing dance.
   */
  wasGenerated?: boolean

  /** Identifier / name of the new agent. */
  agentType?: string

  /** `description` frontmatter (TypeScript upstream calls this `whenToUse`). */
  whenToUse?: string

  /** Markdown body for the agent's system prompt. */
  systemPrompt?: string

  /**
   * Tool allow-list picked via `ToolSelector`. `undefined` = inherit all
   * tools (matches upstream semantic); `[]` = no tools selected.
   */
  selectedTools?: string[]

  /** Optional model override (`"sonnet" | "opus" | "haiku" | "inherit" | ""`). */
  model?: string

  /** Named display color. */
  color?: string

  /** Memory scope selected by the (gated) `MemoryStep`. */
  memory?: AgentMemoryScope

  /** Permission mode — currently not surfaced via a dedicated step, but
   * threaded through so callers can preset it via code. */
  permissionMode?: AgentPermissionMode
}

export type WizardStepComponent = React.ComponentType<{}>

/**
 * Contract the steps use to navigate. Mirrors upstream `useWizard()`
 * but trimmed to the two operations this project actually needs.
 */
export interface WizardApi<T> {
  /** Merge a partial update into the shared state. */
  updateWizardData: (patch: Partial<T>) => void
  /** Full state snapshot. */
  wizardData: T
  /** Advance to the next step in the flat step array. */
  goNext: () => void
  /** Return to the previous step. */
  goBack: () => void
  /** Jump to an arbitrary index (upstream uses this to skip steps). */
  goToStep: (index: number) => void
  /** Cancel the wizard entirely; parent decides what "cancel" does. */
  cancel: () => void
}
