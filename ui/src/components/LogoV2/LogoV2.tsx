import React from 'react'
import { c } from '../../theme.js'
import { truncate } from '../../utils.js'
import { stringWidth } from '../string-width.js'
import { CondensedLogo } from './CondensedLogo.js'
import { Clawd } from './Clawd.js'
import { EmergencyTip } from './EmergencyTip.js'
import { VoiceModeNotice } from './VoiceModeNotice.js'
import { Opus1mMergeNotice } from './Opus1mMergeNotice.js'
import { GateOverridesWarning } from './GateOverridesWarning.js'
import { ExperimentEnrollmentNotice } from './ExperimentEnrollmentNotice.js'
import { ChannelsNotice, type ChannelsNoticeStatus } from './ChannelsNotice.js'
import { FeedColumn } from './FeedColumn.js'
import type { FeedConfig } from './Feed.js'

/**
 * Top-of-conversation welcome frame \u2014 bordered Claude Code card with
 * Clawd on the left, info on the right, and a rotating feed column on
 * the far right when there is screen space.
 *
 * OpenTUI-native port of the upstream `LogoV2/LogoV2`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/LogoV2.tsx`).
 * Upstream combined ~20 cross-module reads (global config, main-loop
 * model, bootstrap state, GrowthBook flags, sandbox, debug mode, tmux
 * env, announcement rotation, KAIROS channel allowlist, voice mode,
 * Opus-1m promo, feed builders). The Lite port takes a flat
 * `LogoV2ViewData` snapshot; the Rust backend assembles the data and
 * the UI stays presentational.
 */

const LEFT_PANEL_MAX_WIDTH = 50

export type LogoV2LayoutMode = 'horizontal' | 'vertical' | 'compact'

export type LogoV2ViewData = {
  version: string
  cwd: string
  username?: string
  modelDisplayName: string
  billingType: string
  organizationName?: string | null
  agentName?: string | null
  columns: number
  /** Pre-computed layout mode from `getLayoutMode()` in upstream. */
  layoutMode: LogoV2LayoutMode
  /** When true, render the condensed header instead of the full card. */
  condensed: boolean
  /** Right-column feed (recent activity / what's new / upsells). */
  feeds?: FeedConfig[]
  /** Status for the KAIROS channels notice. */
  channels?: ChannelsNoticeStatus
  voiceMode?: { visible: boolean; reducedMotion?: boolean }
  opus1mMerge?: { visible: boolean; reducedMotion?: boolean }
  /** GrowthBook-driven tip of the feed. */
  emergencyTip?: {
    tip: string
    color?: 'dim' | 'warning' | 'error'
  }
  debug?: {
    enabled: boolean
    /** Display path for the debug log. */
    logPath?: string
    toStderr?: boolean
  }
  tmux?: {
    session: string
    detachHint: string
  }
  announcement?: string | null
  showSandboxStatus?: boolean
  guestPasses?: { reward?: string | null }
  overageCredit?: { amount?: string | null }
  appleTerminal?: boolean
  animated?: boolean
  /** When true, expose the ANT-only notices (gate overrides, enrollment). */
  showAntOnly?: boolean
}

function formatWelcomeMessage(username?: string): string {
  return username ? `Welcome back, ${username}!` : 'Welcome to Claude Code'
}

function truncatePath(path: string, width: number): string {
  if (path.length <= width) return path
  return '\u2026' + path.slice(path.length - Math.max(0, width - 1))
}

function calculateLayoutDimensions(
  columns: number,
  layoutMode: LogoV2LayoutMode,
  optimalLeftWidth: number,
): { leftWidth: number; rightWidth: number } {
  if (layoutMode !== 'horizontal') {
    return { leftWidth: columns - 4, rightWidth: 0 }
  }
  const left = Math.min(optimalLeftWidth, LEFT_PANEL_MAX_WIDTH)
  const right = Math.max(10, columns - left - 5)
  return { leftWidth: left, rightWidth: right }
}

function calculateOptimalLeftWidth(
  welcome: string,
  cwdLine: string,
  modelLine: string,
): number {
  return (
    Math.min(
      LEFT_PANEL_MAX_WIDTH,
      Math.max(stringWidth(welcome), stringWidth(cwdLine), stringWidth(modelLine)) + 4,
    ) + 2
  )
}

export function LogoV2({ data }: { data: LogoV2ViewData }) {
  const {
    version,
    cwd,
    username,
    modelDisplayName,
    billingType,
    organizationName,
    agentName,
    columns,
    layoutMode,
    condensed,
    feeds = [],
    channels,
    voiceMode,
    opus1mMerge,
    emergencyTip,
    debug,
    tmux,
    announcement,
    showSandboxStatus,
    guestPasses,
    overageCredit,
    appleTerminal,
    animated,
    showAntOnly,
  } = data

  const condensedData = {
    version,
    cwd,
    modelDisplayName,
    billingType,
    agentName,
    columns,
    animated,
    appleTerminal,
    guestPasses,
    overageCredit,
  }

  if (condensed) {
    return (
      <>
        <CondensedLogo data={condensedData} />
        {voiceMode && <VoiceModeNotice {...voiceMode} />}
        {opus1mMerge && <Opus1mMergeNotice {...opus1mMerge} />}
        {channels && <ChannelsNotice status={channels} />}
        {debug?.enabled && (
          <box paddingLeft={2} flexDirection="column">
            <text fg={c.warning}>Debug mode enabled</text>
            <text fg={c.dim}>
              Logging to: {debug.toStderr ? 'stderr' : debug.logPath ?? '<unknown>'}
            </text>
          </box>
        )}
        {emergencyTip && (
          <EmergencyTip tip={emergencyTip.tip} color={emergencyTip.color} />
        )}
        {tmux && (
          <box paddingLeft={2} flexDirection="column">
            <text fg={c.dim}>tmux session: {tmux.session}</text>
            <text fg={c.dim}>{tmux.detachHint}</text>
          </box>
        )}
        {announcement && (
          <box paddingLeft={2} flexDirection="column">
            {organizationName && (
              <text fg={c.dim}>Message from {organizationName}:</text>
            )}
            <text>{announcement}</text>
          </box>
        )}
        {showAntOnly && <GateOverridesWarning />}
        {showAntOnly && <ExperimentEnrollmentNotice />}
      </>
    )
  }

  const welcomeMessage = formatWelcomeMessage(username)
  const modelLine = organizationName
    ? `${modelDisplayName} \u00B7 ${billingType} \u00B7 ${organizationName}`
    : `${modelDisplayName} \u00B7 ${billingType}`
  const separator = ' \u00B7 '
  const atPrefix = '@'
  const cwdAvailableWidth = agentName
    ? LEFT_PANEL_MAX_WIDTH - atPrefix.length - stringWidth(agentName) - separator.length
    : LEFT_PANEL_MAX_WIDTH
  const truncatedCwd = truncatePath(cwd, Math.max(cwdAvailableWidth, 10))
  const cwdLine = agentName ? `@${agentName} \u00B7 ${truncatedCwd}` : truncatedCwd
  const optimalLeftWidth = calculateOptimalLeftWidth(welcomeMessage, cwdLine, modelLine)
  const { leftWidth, rightWidth } = calculateLayoutDimensions(
    columns,
    layoutMode,
    optimalLeftWidth,
  )

  if (layoutMode === 'compact') {
    const displayModel = truncate(modelDisplayName, leftWidth)
    return (
      <>
        <box
          flexDirection="column"
          borderStyle="rounded"
          borderColor={c.accent}
          title="Claude Code"
          titleAlignment="left"
          paddingX={1}
          paddingY={1}
          alignItems="center"
          width={columns}
        >
          <text>
            <strong>{welcomeMessage}</strong>
          </text>
          <box marginTop={1} marginBottom={1}>
            <Clawd appleTerminal={appleTerminal} />
          </box>
          <text fg={c.dim}>{displayModel}</text>
          <text fg={c.dim}>{billingType}</text>
          <text fg={c.dim}>{cwdLine}</text>
        </box>
        {voiceMode && <VoiceModeNotice {...voiceMode} />}
        {opus1mMerge && <Opus1mMergeNotice {...opus1mMerge} />}
        {channels && <ChannelsNotice status={channels} />}
        {showSandboxStatus && (
          <box marginTop={1}>
            <text fg={c.warning}>
              Your bash commands will be sandboxed. Disable with /sandbox.
            </text>
          </box>
        )}
        {showAntOnly && <GateOverridesWarning />}
        {showAntOnly && <ExperimentEnrollmentNotice />}
      </>
    )
  }

  return (
    <>
      <box
        flexDirection="column"
        borderStyle="rounded"
        borderColor={c.accent}
        title={`Claude Code v${version}`}
        titleAlignment="left"
      >
        <box
          flexDirection={layoutMode === 'horizontal' ? 'row' : 'column'}
          paddingX={1}
          gap={1}
        >
          <box
            flexDirection="column"
            width={leftWidth}
            justifyContent="space-between"
            alignItems="center"
            minHeight={9}
          >
            <box marginTop={1}>
              <text>
                <strong>{welcomeMessage}</strong>
              </text>
            </box>
            <Clawd appleTerminal={appleTerminal} />
            <box flexDirection="column" alignItems="center">
              <text fg={c.dim}>{modelLine}</text>
              <text fg={c.dim}>{cwdLine}</text>
            </box>
          </box>
          {layoutMode === 'horizontal' && (
            <>
              <box
                flexDirection="column"
                border={['left']}
                borderStyle="single"
                borderColor={c.accent}
              />
              <FeedColumn feeds={feeds} maxWidth={rightWidth} />
            </>
          )}
        </box>
      </box>
      {voiceMode && <VoiceModeNotice {...voiceMode} />}
      {opus1mMerge && <Opus1mMergeNotice {...opus1mMerge} />}
      {channels && <ChannelsNotice status={channels} />}
      {debug?.enabled && (
        <box paddingLeft={2} flexDirection="column">
          <text fg={c.warning}>Debug mode enabled</text>
          <text fg={c.dim}>
            Logging to: {debug.toStderr ? 'stderr' : debug.logPath ?? '<unknown>'}
          </text>
        </box>
      )}
      {emergencyTip && (
        <EmergencyTip tip={emergencyTip.tip} color={emergencyTip.color} />
      )}
      {tmux && (
        <box paddingLeft={2} flexDirection="column">
          <text fg={c.dim}>tmux session: {tmux.session}</text>
          <text fg={c.dim}>{tmux.detachHint}</text>
        </box>
      )}
      {announcement && (
        <box paddingLeft={2} flexDirection="column">
          {organizationName && (
            <text fg={c.dim}>Message from {organizationName}:</text>
          )}
          <text>{announcement}</text>
        </box>
      )}
      {showSandboxStatus && (
        <box paddingLeft={2}>
          <text fg={c.warning}>
            Your bash commands will be sandboxed. Disable with /sandbox.
          </text>
        </box>
      )}
      {showAntOnly && <GateOverridesWarning />}
      {showAntOnly && <ExperimentEnrollmentNotice />}
    </>
  )
}
