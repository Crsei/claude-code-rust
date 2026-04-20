import React from 'react'
import type { TeamMemberInfo } from '../ipc/protocol.js'
import type { TeamState } from '../store/app-state.js'
import { useAppState } from '../store/app-store.js'

// ---------------------------------------------------------------------------
// Color palette for teammate "color" tags coming from the backend.
// Matches the logical names assigned by `src/teams/helpers.rs::assign_color`.
// ---------------------------------------------------------------------------

const colorTags: Record<string, string> = {
  red: '#F38BA8',
  blue: '#89B4FA',
  green: '#A6E3A1',
  yellow: '#F9E2AF',
  purple: '#CBA6F7',
  orange: '#FAB387',
  pink: '#F5C2E7',
  cyan: '#94E2D5',
}

function colorFor(tag: string | undefined): string {
  if (!tag) return '#CDD6F4'
  return colorTags[tag.toLowerCase()] ?? '#CDD6F4'
}

// ---------------------------------------------------------------------------
// Single teammate row
// ---------------------------------------------------------------------------

function MemberRow({ member }: { member: TeamMemberInfo & { color?: string } }) {
  const statusIcon = member.is_active ? '●' : '○'
  const statusColor = member.is_active ? '#A6E3A1' : '#6C7086'
  const unreadLabel =
    member.unread_messages > 0 ? ` +${member.unread_messages}` : ''
  const roleLabel = member.role ? ` [${member.role}]` : ''

  return (
    <text>
      {'  '}
      <span fg={statusColor}>{statusIcon}</span>
      {' '}
      <span fg={colorFor((member as { color?: string }).color)}>{member.agent_name}</span>
      <span fg="#6C7086">{roleLabel}</span>
      {member.unread_messages > 0 && (
        <span fg="#F9E2AF">{unreadLabel}</span>
      )}
    </text>
  )
}

// ---------------------------------------------------------------------------
// Recent-message row
// ---------------------------------------------------------------------------

function RecentMessageRow({
  from,
  to,
  summary,
}: {
  from: string
  to: string
  summary: string
}) {
  const truncated = summary.length > 60 ? summary.slice(0, 57) + '...' : summary
  return (
    <text>
      {'    '}
      <span fg="#6C7086">{from} → {to}:</span>{' '}
      <span fg="#CDD6F4">{truncated}</span>
    </text>
  )
}

// ---------------------------------------------------------------------------
// Single team section
// ---------------------------------------------------------------------------

function TeamSection({ team }: { team: TeamState }) {
  const memberCount = team.members.length
  const unreadTotal = team.members.reduce(
    (acc, m) => acc + (m.unread_messages ?? 0),
    0,
  )
  const activeCount = team.members.filter(m => m.is_active).length

  return (
    <box flexDirection="column">
      <text>
        <span fg="#89B4FA">{team.name}</span>
        <span fg="#6C7086">
          {' '}({activeCount}/{memberCount} active
          {unreadTotal > 0 ? `, ${unreadTotal} unread` : ''}
          {team.pendingMessages > 0 ? `, ${team.pendingMessages} pending` : ''}
          )
        </span>
      </text>
      {team.members.length === 0 && (
        <text>{'  '}<span fg="#6C7086">(no members yet — use /team spawn or the TeamSpawn tool)</span></text>
      )}
      {team.members.map(m => (
        <MemberRow key={m.agent_id} member={m} />
      ))}
      {team.recentMessages.length > 0 && (
        <>
          <text>{'  '}<span fg="#6C7086">recent:</span></text>
          {team.recentMessages.slice(-3).map((msg, idx) => (
            <RecentMessageRow
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

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------

export function TeamPanel() {
  const { teams } = useAppState()
  const teamList = Object.values(teams)
  if (teamList.length === 0) return null

  const totalMembers = teamList.reduce((acc, t) => acc + t.members.length, 0)
  const title =
    teamList.length === 1
      ? `Team · ${teamList[0]!.name} (${totalMembers} member${totalMembers === 1 ? '' : 's'})`
      : `Teams (${teamList.length} teams, ${totalMembers} members)`

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
