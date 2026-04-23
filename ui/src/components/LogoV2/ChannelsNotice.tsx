import React from 'react'
import { c } from '../../theme.js'

/**
 * "Listening for channel messages from\u2026" notice shown when the user
 * launched with `--channels` (KAIROS feature).
 *
 * OpenTUI-native port of the upstream `LogoV2/ChannelsNotice`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/ChannelsNotice.tsx`).
 * The upstream component inspected bootstrap state, GrowthBook flags,
 * MCP configs, and installed plugins to decide which of four branches
 * to render. The Lite port accepts a flat `ChannelsNoticeStatus` so the
 * Rust backend produces the snapshot once (KAIROS lives Rust-side) and
 * the UI stays a pure renderer.
 */

export type ChannelEntryDisplay = {
  kind: 'plugin' | 'server'
  name: string
  /** Required for `kind === 'plugin'`. */
  marketplace?: string
  dev?: boolean
}

export type ChannelsNoticeUnmatched = {
  entry: ChannelEntryDisplay
  why: string
}

export type ChannelsNoticeStatus =
  | { state: 'hidden' }
  | {
      state: 'disabled' | 'noAuth' | 'policyBlocked' | 'listening'
      channels: ChannelEntryDisplay[]
      flag: string
      unmatched?: ChannelsNoticeUnmatched[]
    }

function formatEntry(entry: ChannelEntryDisplay): string {
  return entry.kind === 'plugin'
    ? `plugin:${entry.name}@${entry.marketplace ?? ''}`
    : `server:${entry.name}`
}

export function ChannelsNotice({
  status,
}: {
  status: ChannelsNoticeStatus
}) {
  if (status.state === 'hidden') return null
  const list = status.channels.map(formatEntry).join(', ')
  const flag = status.flag

  if (status.state === 'disabled') {
    return (
      <box paddingLeft={2} flexDirection="column">
        <text fg={c.error}>{`${flag} ignored (${list})`}</text>
        <text fg={c.dim}>Channels are not currently available</text>
      </box>
    )
  }

  if (status.state === 'noAuth') {
    return (
      <box paddingLeft={2} flexDirection="column">
        <text fg={c.error}>{`${flag} ignored (${list})`}</text>
        <text fg={c.dim}>
          Channels require claude.ai authentication \u00B7 run /login, then restart
        </text>
      </box>
    )
  }

  if (status.state === 'policyBlocked') {
    return (
      <box paddingLeft={2} flexDirection="column">
        <text fg={c.error}>{`${flag} blocked by org policy (${list})`}</text>
        <text fg={c.dim}>Inbound messages will be silently dropped</text>
        <text fg={c.dim}>
          Have an administrator set channelsEnabled: true in managed settings to
          enable
        </text>
        {status.unmatched?.map((u, i) => (
          <text key={`u-${i}`} fg={c.warning}>
            {`${formatEntry(u.entry)} \u00B7 ${u.why}`}
          </text>
        ))}
      </box>
    )
  }

  return (
    <box paddingLeft={2} flexDirection="column">
      <text fg={c.error}>{`Listening for channel messages from: ${list}`}</text>
      <text fg={c.dim}>
        Experimental \u00B7 inbound messages will be pushed into this session,
        this carries prompt injection risks. Restart Claude Code without {flag}{' '}
        to disable.
      </text>
      {status.unmatched?.map((u, i) => (
        <text key={`u-${i}`} fg={c.warning}>
          {`${formatEntry(u.entry)} \u00B7 ${u.why}`}
        </text>
      ))}
    </box>
  )
}
