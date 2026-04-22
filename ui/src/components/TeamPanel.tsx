import React from 'react'
import type { TeamState } from '../store/app-state.js'
import { useAppState } from '../store/app-store.js'
import { TeamMemberCard, summarizeTeam, summarizeTeams } from './panels/index.js'

/**
 * Active-team panel. Consumes `summarizeTeam` / `summarizeTeams` for
 * aggregation (pure helpers under `./panels/`) so the header logic
 * stays unit-testable, and delegates each member row to
 * `TeamMemberCard`.
 */

const MAX_RECENT_MESSAGES_SHOWN = 3

function TruncatedSummary({
  from,
  to,
  summary,
}: {
  from: string
  to: string
  summary: string
}) {
  const truncated = summary.length > 60 ? `${summary.slice(0, 57)}...` : summary
  return (
    <text>
      {'    '}
      <span fg="#6C7086">
        {from} → {to}:
      </span>{' '}
      <span fg="#CDD6F4">{truncated}</span>
    </text>
  )
}

function TeamSection({ team }: { team: TeamState }) {
  const s = summarizeTeam(team)
  const headerSuffix = [
    `${s.activeMembers}/${s.totalMembers} active`,
    s.unreadTotal > 0 ? `${s.unreadTotal} unread` : null,
    s.pendingMessages > 0 ? `${s.pendingMessages} pending` : null,
  ]
    .filter(Boolean)
    .join(', ')

  const recent = team.recentMessages.slice(-MAX_RECENT_MESSAGES_SHOWN)
  const totalRecent = team.recentMessages.length
  const hiddenRecent = Math.max(0, totalRecent - recent.length)

  return (
    <box flexDirection="column">
      <text>
        <span fg="#89B4FA">{team.name}</span>
        <span fg="#6C7086"> ({headerSuffix})</span>
      </text>
      {team.members.length === 0 && (
        <text>
          {'  '}
          <span fg="#6C7086">(no members yet — use /team spawn or the TeamSpawn tool)</span>
        </text>
      )}
      {team.members.map(m => (
        <TeamMemberCard key={m.agent_id} member={m} />
      ))}
      {recent.length > 0 && (
        <>
          <text>
            {'  '}
            <span fg="#6C7086">recent{hiddenRecent > 0 ? ` (latest ${recent.length} of ${totalRecent})` : ''}:</span>
          </text>
          {recent.map((msg, idx) => (
            <TruncatedSummary
              key={`${team.name}-${msg.timestamp}-${idx}`}
              from={msg.from}
              to={msg.to}
              summary={msg.summary}
            />
          ))}
        </>
      )}
    </box>
  )
}

export function TeamPanel() {
  const { teams } = useAppState()
  const teamList = Object.values(teams)
  if (teamList.length === 0) return null

  const rollup = summarizeTeams(teamList)
  const title =
    rollup.teamCount === 1
      ? `Team · ${teamList[0]!.name} (${rollup.totalMembers} member${rollup.totalMembers === 1 ? '' : 's'})`
      : `Teams (${rollup.teamCount} teams, ${rollup.totalMembers} members${rollup.unreadTotal > 0 ? `, ${rollup.unreadTotal} unread` : ''})`

  return (
    <box
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor="#45475A"
      paddingX={1}
      title={title}
      titleAlignment="left"
      gap={0}
    >
      {teamList.map(team => (
        <TeamSection key={team.name} team={team} />
      ))}
    </box>
  )
}
