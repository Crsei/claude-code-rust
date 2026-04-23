import React from 'react'
import { c } from '../../theme.js'
import {
  isSandboxingEnabled,
  shouldAllowManagedSandboxDomainsOnly,
  useSandboxAdapter,
} from './sandbox-adapter.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/sandbox/SandboxConfigTab.tsx`.
 *
 * Read-only view of the active sandbox config. Upstream calls into the
 * singleton `SandboxManager`; we pull the same fields from the
 * `SandboxAdapterContext` so the component stays pure and parents
 * decide where the data comes from.
 */

export function SandboxConfigTab() {
  const { settings } = useSandboxAdapter()
  const enabled = isSandboxingEnabled(settings)

  const depCheck = settings.dependencyCheck
  const warningsNote =
    depCheck.warnings.length > 0 ? (
      <box marginTop={1} flexDirection="column">
        {depCheck.warnings.map((w, i) => (
          <text key={i} fg={c.dim} selectable>
            {w}
          </text>
        ))}
      </box>
    ) : null

  if (!enabled) {
    return (
      <box flexDirection="column" paddingY={1}>
        <text fg={c.dim}>Sandbox is not enabled</text>
        {warningsNote}
      </box>
    )
  }

  const fsRead = settings.fsRead
  const fsWrite = settings.fsWrite
  const network = settings.network
  const hasNetworkRules =
    (network.allowedHosts && network.allowedHosts.length > 0) ||
    (network.deniedHosts && network.deniedHosts.length > 0)

  return (
    <box flexDirection="column" paddingY={1}>
      <Section title="Excluded Commands:">
        <text fg={c.dim} selectable>
          {settings.excludedCommands.length > 0
            ? settings.excludedCommands.join(', ')
            : 'None'}
        </text>
      </Section>

      {fsRead.denyOnly.length > 0 && (
        <Section title="Filesystem Read Restrictions:" marginTop>
          <text fg={c.dim} selectable>Denied: {fsRead.denyOnly.join(', ')}</text>
          {fsRead.allowWithinDeny && fsRead.allowWithinDeny.length > 0 && (
            <text fg={c.dim} selectable>
              Allowed within denied: {fsRead.allowWithinDeny.join(', ')}
            </text>
          )}
        </Section>
      )}

      {fsWrite.allowOnly.length > 0 && (
        <Section title="Filesystem Write Restrictions:" marginTop>
          <text fg={c.dim} selectable>Allowed: {fsWrite.allowOnly.join(', ')}</text>
          {fsWrite.denyWithinAllow.length > 0 && (
            <text fg={c.dim} selectable>
              Denied within allowed: {fsWrite.denyWithinAllow.join(', ')}
            </text>
          )}
        </Section>
      )}

      {hasNetworkRules && (
        <Section
          title={`Network Restrictions${shouldAllowManagedSandboxDomainsOnly(settings) ? ' (Managed)' : ''}:`}
          marginTop
        >
          {network.allowedHosts && network.allowedHosts.length > 0 && (
            <text fg={c.dim} selectable>
              Allowed: {network.allowedHosts.join(', ')}
            </text>
          )}
          {network.deniedHosts && network.deniedHosts.length > 0 && (
            <text fg={c.dim} selectable>
              Denied: {network.deniedHosts.join(', ')}
            </text>
          )}
        </Section>
      )}

      {settings.allowUnixSockets.length > 0 && (
        <Section title="Allowed Unix Sockets:" marginTop>
          <text fg={c.dim} selectable>
            {settings.allowUnixSockets.join(', ')}
          </text>
        </Section>
      )}

      {settings.linuxGlobPatternWarnings.length > 0 && (
        <box marginTop={1} flexDirection="column">
          <text fg={c.warning}>
            <strong>\u26A0 Warning: Glob patterns not fully supported on Linux</strong>
          </text>
          <text fg={c.dim} selectable>
            The following patterns will be ignored:{' '}
            {settings.linuxGlobPatternWarnings.slice(0, 3).join(', ')}
            {settings.linuxGlobPatternWarnings.length > 3 &&
              ` (${settings.linuxGlobPatternWarnings.length - 3} more)`}
          </text>
        </box>
      )}

      {warningsNote}
    </box>
  )
}

function Section({
  title,
  marginTop,
  children,
}: {
  title: string
  marginTop?: boolean
  children: React.ReactNode
}) {
  return (
    <box marginTop={marginTop ? 1 : 0} flexDirection="column">
      <text fg={c.accent}>
        <strong>{title}</strong>
      </text>
      {children}
    </box>
  )
}
