import React from 'react'
import { c } from '../theme.js'
import { formatTokens } from '../utils.js'
import {
  ContextSuggestions,
  type ContextSuggestion,
} from './ContextSuggestions.js'

/**
 * `/context` visualization \u2014 token grid + legend + per-source
 * breakdowns (memory files, MCP tools, agents, skills).
 *
 * OpenTUI-native port of the upstream `ContextVisualization`
 * (`ui/examples/upstream-patterns/src/components/ContextVisualization.tsx`).
 * Upstream depended on `analyzeContext`, `generateContextSuggestions`,
 * the `bun:bundle` feature gate, and the settings-source display
 * helpers. The Lite port keeps the same visual shape but expects the
 * caller to hand in a pre-computed `ContextVisualizationData` object.
 */

const RESERVED_CATEGORY_NAME = 'Autocompact buffer'

export type ContextGridSquare = {
  categoryName: string
  color?: string
  squareFullness: number
}

export type ContextCategory = {
  name: string
  color?: string
  tokens: number
  isDeferred?: boolean
}

export type ContextMcpTool = {
  name: string
  tokens: number
  isLoaded: boolean
}

export type ContextAgent = {
  agentType: string
  source: string
  tokens: number
}

export type ContextMemoryFile = {
  path: string
  displayPath?: string
  tokens: number
}

export type ContextSkill = {
  name: string
  source: string
  tokens: number
}

export type ContextVisualizationData = {
  model: string
  categories: ContextCategory[]
  totalTokens: number
  rawMaxTokens: number
  percentage: number
  gridRows: ContextGridSquare[][]
  mcpTools: ContextMcpTool[]
  deferredBuiltinTools?: Array<{ name: string; tokens: number; isLoaded: boolean }>
  systemTools?: Array<{ name: string; tokens: number }>
  systemPromptSections?: Array<{ name: string; tokens: number }>
  agents: ContextAgent[]
  skills?: { tokens: number; skillFrontmatter: ContextSkill[] }
  memoryFiles: ContextMemoryFile[]
  messageBreakdown?: {
    toolCallTokens: number
    toolResultTokens: number
    attachmentTokens: number
    assistantMessageTokens: number
    userMessageTokens: number
    toolCallsByType: Array<{ name: string; callTokens: number; resultTokens: number }>
    attachmentsByType: Array<{ name: string; tokens: number }>
  }
  suggestions?: ContextSuggestion[]
  /** Set to 'ant' to include the upstream ANT-only sections. */
  userType?: string
}

const SOURCE_DISPLAY_ORDER = ['Project', 'User', 'Managed', 'Plugin', 'Built-in']

function groupBySource<T extends { source: string; tokens: number }>(
  items: T[],
): Map<string, T[]> {
  const groups = new Map<string, T[]>()
  for (const item of items) {
    const existing = groups.get(item.source) ?? []
    existing.push(item)
    groups.set(item.source, existing)
  }
  for (const [key, group] of groups.entries()) {
    groups.set(key, [...group].sort((a, b) => b.tokens - a.tokens))
  }
  const ordered = new Map<string, T[]>()
  for (const source of SOURCE_DISPLAY_ORDER) {
    const group = groups.get(source)
    if (group) ordered.set(source, group)
  }
  for (const [key, group] of groups.entries()) {
    if (!ordered.has(key)) ordered.set(key, group)
  }
  return ordered
}

type Props = {
  data: ContextVisualizationData
}

export function ContextVisualization({ data }: Props) {
  const {
    categories,
    totalTokens,
    rawMaxTokens,
    percentage,
    gridRows,
    model,
    memoryFiles,
    mcpTools,
    deferredBuiltinTools = [],
    systemTools,
    systemPromptSections,
    agents,
    skills,
    messageBreakdown,
    userType,
    suggestions = [],
  } = data

  const visibleCategories = categories.filter(
    cat =>
      cat.tokens > 0 &&
      cat.name !== 'Free space' &&
      cat.name !== RESERVED_CATEGORY_NAME &&
      !cat.isDeferred,
  )
  const hasDeferredMcpTools = categories.some(
    cat => cat.isDeferred && cat.name.includes('MCP'),
  )
  const hasDeferredBuiltinTools = deferredBuiltinTools.length > 0
  const autocompactCategory = categories.find(
    cat => cat.name === RESERVED_CATEGORY_NAME,
  )
  const freeSpace = categories.find(c => c.name === 'Free space')
  const isAnt = userType === 'ant'

  return (
    <box flexDirection="column" paddingLeft={1}>
      <text>
        <strong>Context Usage</strong>
      </text>
      <box flexDirection="row" gap={2}>
        <box flexDirection="column" flexShrink={0}>
          {gridRows.map((row, rowIndex) => (
            <box key={rowIndex} flexDirection="row">
              {row.map((square, colIndex) => {
                if (square.categoryName === 'Free space') {
                  return (
                    <text key={colIndex} fg={c.dim}>
                      {'\u26F6 '}
                    </text>
                  )
                }
                if (square.categoryName === RESERVED_CATEGORY_NAME) {
                  return (
                    <text key={colIndex} fg={square.color}>
                      {'\u26DD '}
                    </text>
                  )
                }
                return (
                  <text key={colIndex} fg={square.color}>
                    {square.squareFullness >= 0.7 ? '\u26C1 ' : '\u26C0 '}
                  </text>
                )
              })}
            </box>
          ))}
        </box>
        <box flexDirection="column" flexShrink={0}>
          <text fg={c.dim}>
            {model} \u00B7 {formatTokens(totalTokens)}/{formatTokens(rawMaxTokens)} tokens ({percentage}%)
          </text>
          <text>{' '}</text>
          <text>
            <em>
              <span fg={c.dim}>Estimated usage by category</span>
            </em>
          </text>
          {visibleCategories.map((cat, index) => {
            const tokenDisplay = formatTokens(cat.tokens)
            const percentDisplay = cat.isDeferred
              ? 'N/A'
              : `${((cat.tokens / rawMaxTokens) * 100).toFixed(1)}%`
            const isReserved = cat.name === RESERVED_CATEGORY_NAME
            const symbol = cat.isDeferred ? ' ' : isReserved ? '\u26DD' : '\u26C1'
            return (
              <box key={`${cat.name}-${index}`} flexDirection="row">
                <text fg={cat.color}>{symbol}</text>
                <text>{` ${cat.name}: `}</text>
                <text fg={c.dim}>{tokenDisplay} tokens ({percentDisplay})</text>
              </box>
            )
          })}
          {(freeSpace?.tokens ?? 0) > 0 && freeSpace && (
            <box flexDirection="row">
              <text fg={c.dim}>\u26F6</text>
              <text>{` Free space: `}</text>
              <text fg={c.dim}>
                {formatTokens(freeSpace.tokens)} (
                {((freeSpace.tokens / rawMaxTokens) * 100).toFixed(1)}%)
              </text>
            </box>
          )}
          {autocompactCategory && autocompactCategory.tokens > 0 && (
            <box flexDirection="row">
              <text fg={autocompactCategory.color}>\u26DD</text>
              <text fg={c.dim}>{` ${autocompactCategory.name}: `}</text>
              <text fg={c.dim}>
                {formatTokens(autocompactCategory.tokens)} tokens (
                {((autocompactCategory.tokens / rawMaxTokens) * 100).toFixed(1)}%)
              </text>
            </box>
          )}
        </box>
      </box>

      <box flexDirection="column">
        {mcpTools.length > 0 && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>MCP tools</strong>
              <span fg={c.dim}>
                {` \u00B7 /mcp${hasDeferredMcpTools ? ' (loaded on-demand)' : ''}`}
              </span>
            </text>
            {mcpTools.some(t => t.isLoaded) && (
              <box flexDirection="column" marginTop={1}>
                <text fg={c.dim}>Loaded</text>
                {mcpTools
                  .filter(t => t.isLoaded)
                  .map((tool, i) => (
                    <box key={`loaded-${i}`} flexDirection="row">
                      <text>\u2514 {tool.name}: </text>
                      <text fg={c.dim}>{formatTokens(tool.tokens)} tokens</text>
                    </box>
                  ))}
              </box>
            )}
            {hasDeferredMcpTools && mcpTools.some(t => !t.isLoaded) && (
              <box flexDirection="column" marginTop={1}>
                <text fg={c.dim}>Available</text>
                {mcpTools
                  .filter(t => !t.isLoaded)
                  .map((tool, i) => (
                    <box key={`avail-${i}`} flexDirection="row">
                      <text fg={c.dim}>\u2514 {tool.name}</text>
                    </box>
                  ))}
              </box>
            )}
            {!hasDeferredMcpTools &&
              mcpTools.map((tool, i) => (
                <box key={`all-${i}`} flexDirection="row">
                  <text>\u2514 {tool.name}: </text>
                  <text fg={c.dim}>{formatTokens(tool.tokens)} tokens</text>
                </box>
              ))}
          </box>
        )}

        {isAnt && ((systemTools && systemTools.length > 0) || hasDeferredBuiltinTools) && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>[ANT-ONLY] System tools</strong>
              {hasDeferredBuiltinTools && (
                <span fg={c.dim}> (some loaded on-demand)</span>
              )}
            </text>
            <box flexDirection="column" marginTop={1}>
              <text fg={c.dim}>Loaded</text>
              {systemTools?.map((tool, i) => (
                <box key={`sys-${i}`} flexDirection="row">
                  <text>\u2514 {tool.name}: </text>
                  <text fg={c.dim}>{formatTokens(tool.tokens)} tokens</text>
                </box>
              ))}
              {deferredBuiltinTools
                .filter(t => t.isLoaded)
                .map((tool, i) => (
                  <box key={`def-loaded-${i}`} flexDirection="row">
                    <text>\u2514 {tool.name}: </text>
                    <text fg={c.dim}>{formatTokens(tool.tokens)} tokens</text>
                  </box>
                ))}
            </box>
            {hasDeferredBuiltinTools &&
              deferredBuiltinTools.some(t => !t.isLoaded) && (
                <box flexDirection="column" marginTop={1}>
                  <text fg={c.dim}>Available</text>
                  {deferredBuiltinTools
                    .filter(t => !t.isLoaded)
                    .map((tool, i) => (
                      <box key={`def-avail-${i}`} flexDirection="row">
                        <text fg={c.dim}>\u2514 {tool.name}</text>
                      </box>
                    ))}
                </box>
              )}
          </box>
        )}

        {isAnt && systemPromptSections && systemPromptSections.length > 0 && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>[ANT-ONLY] System prompt sections</strong>
            </text>
            {systemPromptSections.map((section, i) => (
              <box key={`sp-${i}`} flexDirection="row">
                <text>\u2514 {section.name}: </text>
                <text fg={c.dim}>{formatTokens(section.tokens)} tokens</text>
              </box>
            ))}
          </box>
        )}

        {agents.length > 0 && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>Custom agents</strong>
              <span fg={c.dim}> \u00B7 /agents</span>
            </text>
            {Array.from(groupBySource(agents).entries()).map(
              ([sourceDisplay, sourceAgents]) => (
                <box key={sourceDisplay} flexDirection="column" marginTop={1}>
                  <text fg={c.dim}>{sourceDisplay}</text>
                  {sourceAgents.map((agent, i) => (
                    <box key={`agent-${i}`} flexDirection="row">
                      <text>\u2514 {agent.agentType}: </text>
                      <text fg={c.dim}>{formatTokens(agent.tokens)} tokens</text>
                    </box>
                  ))}
                </box>
              ),
            )}
          </box>
        )}

        {memoryFiles.length > 0 && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>Memory files</strong>
              <span fg={c.dim}> \u00B7 /memory</span>
            </text>
            {memoryFiles.map((file, i) => (
              <box key={`mem-${i}`} flexDirection="row">
                <text>\u2514 {file.displayPath ?? file.path}: </text>
                <text fg={c.dim}>{formatTokens(file.tokens)} tokens</text>
              </box>
            ))}
          </box>
        )}

        {skills && skills.tokens > 0 && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>Skills</strong>
              <span fg={c.dim}> \u00B7 /skills</span>
            </text>
            {Array.from(groupBySource(skills.skillFrontmatter).entries()).map(
              ([sourceDisplay, sourceSkills]) => (
                <box key={sourceDisplay} flexDirection="column" marginTop={1}>
                  <text fg={c.dim}>{sourceDisplay}</text>
                  {sourceSkills.map((skill, i) => (
                    <box key={`skill-${i}`} flexDirection="row">
                      <text>\u2514 {skill.name}: </text>
                      <text fg={c.dim}>{formatTokens(skill.tokens)} tokens</text>
                    </box>
                  ))}
                </box>
              ),
            )}
          </box>
        )}

        {isAnt && messageBreakdown && (
          <box flexDirection="column" marginTop={1}>
            <text>
              <strong>[ANT-ONLY] Message breakdown</strong>
            </text>
            <box flexDirection="column" marginLeft={1}>
              <text>Tool calls: <span fg={c.dim}>{formatTokens(messageBreakdown.toolCallTokens)} tokens</span></text>
              <text>Tool results: <span fg={c.dim}>{formatTokens(messageBreakdown.toolResultTokens)} tokens</span></text>
              <text>Attachments: <span fg={c.dim}>{formatTokens(messageBreakdown.attachmentTokens)} tokens</span></text>
              <text>Assistant messages (non-tool): <span fg={c.dim}>{formatTokens(messageBreakdown.assistantMessageTokens)} tokens</span></text>
              <text>User messages (non-tool-result): <span fg={c.dim}>{formatTokens(messageBreakdown.userMessageTokens)} tokens</span></text>
            </box>
            {messageBreakdown.toolCallsByType.length > 0 && (
              <box flexDirection="column" marginTop={1}>
                <text><strong>[ANT-ONLY] Top tools</strong></text>
                {messageBreakdown.toolCallsByType.slice(0, 5).map((tool, i) => (
                  <box key={`tool-${i}`} marginLeft={1} flexDirection="row">
                    <text>\u2514 {tool.name}: </text>
                    <text fg={c.dim}>
                      calls {formatTokens(tool.callTokens)}, results {formatTokens(tool.resultTokens)}
                    </text>
                  </box>
                ))}
              </box>
            )}
            {messageBreakdown.attachmentsByType.length > 0 && (
              <box flexDirection="column" marginTop={1}>
                <text><strong>[ANT-ONLY] Top attachments</strong></text>
                {messageBreakdown.attachmentsByType.slice(0, 5).map((att, i) => (
                  <box key={`att-${i}`} marginLeft={1} flexDirection="row">
                    <text>\u2514 {att.name}: </text>
                    <text fg={c.dim}>{formatTokens(att.tokens)} tokens</text>
                  </box>
                ))}
              </box>
            )}
          </box>
        )}
      </box>

      <ContextSuggestions suggestions={suggestions} />
    </box>
  )
}
