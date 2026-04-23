import React, { useMemo } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'

/**
 * Ported from `claude-code-bun/src/components/skills/SkillsMenu.tsx`.
 *
 * Upstream reads the full slash-command registry and filters down to
 * `loadedFrom === 'skills' | 'commands_DEPRECATED' | 'plugin' | 'mcp'`,
 * then groups by `source` (projectSettings, userSettings, etc.). The
 * Rust port surfaces skills through the IPC protocol as a flat list of
 * records — callers pass them in via `skills`, keyed by the same shape
 * upstream uses so the grouping / subtitle logic is verbatim.
 *
 * Everything Ink-specific (`Dialog`, `Box`, `Text`, `ConfigurableShortcutHint`)
 * is replaced with OpenTUI intrinsics and a raw `useKeyboard` listener
 * that dismisses the dialog on Esc.
 */

export type SkillSource =
  | 'projectSettings'
  | 'userSettings'
  | 'policySettings'
  | 'localSettings'
  | 'flagSettings'
  | 'plugin'
  | 'mcp'

export type SkillEntry = {
  /** Stable skill name — e.g. `writing-skills` or `fs:read_file`. */
  name: string
  /** Which settings bucket this skill came from. */
  source: SkillSource
  /** Approximate token cost of the skill's frontmatter (for display). */
  estimatedTokens: number
  /** Optional plugin manifest name when `source === 'plugin'`. */
  pluginName?: string
  /** When true, the skill was loaded from the legacy `commands/` directory. */
  loadedFromDeprecatedCommands?: boolean
}

type Props = {
  skills: SkillEntry[]
  /** Map from `source` to the filesystem path used in the subtitle. The
   *  Rust port computes these paths in the daemon; upstream called
   *  `getSkillsPath(source, 'skills')` which isn't available here. */
  sourcePaths?: Partial<Record<SkillSource, string>>
  /** Map from `source` to the `commands_DEPRECATED` path (used when any
   *  skill in the group came from `commands/`). Upstream derives this from
   *  `getSkillsPath(source, 'commands')`. */
  deprecatedCommandsPaths?: Partial<Record<SkillSource, string>>
  onExit: (result?: string) => void
}

function plural(n: number, noun: string): string {
  return n === 1 ? noun : `${noun}s`
}

function capitalize(source: SkillSource): string {
  if (source === 'policySettings') return 'Policy'
  if (source === 'userSettings') return 'User'
  if (source === 'projectSettings') return 'Project'
  if (source === 'localSettings') return 'Local'
  if (source === 'flagSettings') return 'Flag'
  return source
}

function getSourceTitle(source: SkillSource): string {
  if (source === 'plugin') return 'Plugin skills'
  if (source === 'mcp') return 'MCP skills'
  return `${capitalize(source)} skills`
}

function getSourceSubtitle(
  source: SkillSource,
  skills: SkillEntry[],
  sourcePaths: Props['sourcePaths'],
  deprecatedCommandsPaths: Props['deprecatedCommandsPaths'],
): string | undefined {
  if (source === 'mcp') {
    const servers = [
      ...new Set(
        skills
          .map(s => {
            const idx = s.name.indexOf(':')
            return idx > 0 ? s.name.slice(0, idx) : null
          })
          .filter((n): n is string => n != null),
      ),
    ]
    return servers.length > 0 ? servers.join(', ') : undefined
  }
  const skillsPath = sourcePaths?.[source]
  if (!skillsPath) return undefined
  const hasCommandsSkills = skills.some(s => s.loadedFromDeprecatedCommands)
  const commandsPath = deprecatedCommandsPaths?.[source]
  if (hasCommandsSkills && commandsPath) {
    return `${skillsPath}, ${commandsPath}`
  }
  return skillsPath
}

function getScopeTag(source: SkillSource): { label: string; color: string } | undefined {
  switch (source) {
    case 'projectSettings':
    case 'localSettings':
      return { label: 'local', color: c.warning }
    case 'userSettings':
      return { label: 'global', color: c.info }
    case 'policySettings':
      return { label: 'managed', color: c.accent }
    default:
      return undefined
  }
}

function formatTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return String(n)
}

const GROUP_ORDER: SkillSource[] = [
  'projectSettings',
  'localSettings',
  'userSettings',
  'flagSettings',
  'policySettings',
  'plugin',
  'mcp',
]

export function SkillsMenu({
  skills,
  sourcePaths,
  deprecatedCommandsPaths,
  onExit,
}: Props): React.ReactElement {
  const skillsBySource = useMemo(() => {
    const groups: Record<SkillSource, SkillEntry[]> = {
      projectSettings: [],
      userSettings: [],
      policySettings: [],
      localSettings: [],
      flagSettings: [],
      plugin: [],
      mcp: [],
    }
    for (const skill of skills) {
      if (skill.source in groups) groups[skill.source].push(skill)
    }
    for (const group of Object.values(groups)) {
      group.sort((a, b) => a.name.localeCompare(b.name))
    }
    return groups
  }, [skills])

  const handleCancel = () => {
    onExit('Skills dialog dismissed')
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'escape') handleCancel()
  })

  if (skills.length === 0) {
    return (
      <box
        flexDirection="column"
        borderStyle="rounded"
        borderColor={c.accent}
        paddingX={2}
        paddingY={1}
        title="Skills"
        titleAlignment="center"
      >
        <text fg={c.dim}>No skills found</text>
        <box marginTop={1}>
          <text fg={c.dim}>
            Create skills in .claude/skills/ or ~/.claude/skills/
          </text>
        </box>
        <box marginTop={1}>
          <text fg={c.dim}>
            <em>Esc to close</em>
          </text>
        </box>
      </box>
    )
  }

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
      title="Skills"
      titleAlignment="center"
    >
      <text fg={c.dim}>
        {skills.length} {plural(skills.length, 'skill')}
      </text>

      <box flexDirection="column" marginTop={1}>
        {GROUP_ORDER.map(source => {
          const groupSkills = skillsBySource[source]
          if (groupSkills.length === 0) return null
          const title = getSourceTitle(source)
          const subtitle = getSourceSubtitle(
            source,
            groupSkills,
            sourcePaths,
            deprecatedCommandsPaths,
          )
          return (
            <box key={source} flexDirection="column" marginBottom={1}>
              <box flexDirection="row">
                <text>
                  <strong>
                    <span fg={c.dim}>{title}</span>
                  </strong>
                </text>
                {subtitle ? (
                  <text fg={c.dim}> ({subtitle})</text>
                ) : null}
              </box>
              {groupSkills.map(skill => {
                const tokenDisplay = `~${formatTokens(skill.estimatedTokens)}`
                const scopeTag = getScopeTag(skill.source)
                return (
                  <box
                    key={`${skill.name}-${skill.source}`}
                    flexDirection="row"
                  >
                    <text>{skill.name}</text>
                    {scopeTag ? (
                      <text fg={scopeTag.color}> [{scopeTag.label}]</text>
                    ) : null}
                    <text fg={c.dim}>
                      {skill.pluginName ? ` \u00b7 ${skill.pluginName}` : ''}
                      {' \u00b7 '}
                      {tokenDisplay} description tokens
                    </text>
                  </box>
                )
              })}
            </box>
          )
        })}
      </box>

      <box marginTop={1}>
        <text fg={c.dim}>
          <em>Esc to close</em>
        </text>
      </box>
    </box>
  )
}
