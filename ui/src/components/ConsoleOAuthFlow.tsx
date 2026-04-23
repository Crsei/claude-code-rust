import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * Interactive OAuth / platform selection flow.
 *
 * OpenTUI-native port of the upstream `ConsoleOAuthFlow`
 * (`ui/examples/upstream-patterns/src/components/ConsoleOAuthFlow.tsx`).
 *
 * The upstream component owns a 9-state machine that drives the whole
 * login flow \u2014 platform picker, browser launch, manual code pasting,
 * per-provider form entries (Anthropic-compat / OpenAI / Gemini), key
 * creation, and success / retry surfaces. It depends on services the
 * Lite tree doesn't expose to the frontend (`OAuthService`,
 * `installOAuthTokens`, `getOauthAccountInfo`, `updateSettingsForSource`,
 * analytics, notifications). Those live Rust-side in this port.
 *
 * To keep parity with the upstream UX while respecting our IPC
 * boundary, this port exposes a **presentation-only** surface: callers
 * pass a `status` object describing the current state and handler
 * callbacks, the component renders the appropriate screen. The Rust
 * backend drives the actual OAuth dance and pushes status updates.
 */

export type OAuthProviderKind =
  | 'claudeai'
  | 'console'
  | 'platform'
  | 'custom_platform'
  | 'openai_chat_api'
  | 'gemini_api'

export type OAuthFormField =
  | 'base_url'
  | 'api_key'
  | 'haiku_model'
  | 'sonnet_model'
  | 'opus_model'

export type OAuthFormValues = Record<OAuthFormField, string>

export type ConsoleOAuthStatus =
  | { state: 'idle' }
  | { state: 'platform_setup' }
  | {
      state: 'provider_form'
      provider: 'custom_platform' | 'openai_chat_api' | 'gemini_api'
      activeField: OAuthFormField
      values: OAuthFormValues
    }
  | { state: 'ready_to_start' }
  | { state: 'waiting_for_login'; url: string; showPastePrompt: boolean }
  | { state: 'creating_api_key' }
  | { state: 'about_to_retry' }
  | { state: 'success'; token?: string; email?: string }
  | { state: 'error'; message: string; canRetry: boolean }

type Props = {
  status: ConsoleOAuthStatus
  mode?: 'login' | 'setup-token'
  startingMessage?: string
  forcedMethodMessage?: string | null
  onSelectProvider: (provider: OAuthProviderKind) => void
  onSubmitManualCode: (code: string) => void
  onRetry: () => void
  onCopyUrl: (url: string) => void
  onDone: () => void
}

type OptionEntry = {
  value: OAuthProviderKind
  title: string
  description: string
}

const DEFAULT_OPTIONS: OptionEntry[] = [
  {
    value: 'custom_platform',
    title: 'Anthropic Compatible',
    description: 'Configure your own API endpoint',
  },
  {
    value: 'openai_chat_api',
    title: 'OpenAI Compatible',
    description: 'Ollama, DeepSeek, vLLM, One API, etc.',
  },
  {
    value: 'gemini_api',
    title: 'Gemini API',
    description: 'Google Gemini native REST/SSE',
  },
  {
    value: 'claudeai',
    title: 'Claude account with subscription',
    description: 'Pro, Max, Team, or Enterprise',
  },
  {
    value: 'console',
    title: 'Anthropic Console account',
    description: 'API usage billing',
  },
  {
    value: 'platform',
    title: '3rd-party platform',
    description: 'Amazon Bedrock, Microsoft Foundry, or Vertex AI',
  },
]

const FORM_FIELDS: OAuthFormField[] = [
  'base_url',
  'api_key',
  'haiku_model',
  'sonnet_model',
  'opus_model',
]

const FIELD_LABELS: Record<OAuthFormField, string> = {
  base_url: 'Base URL ',
  api_key: 'API Key  ',
  haiku_model: 'Haiku    ',
  sonnet_model: 'Sonnet   ',
  opus_model: 'Opus     ',
}

function IdleView({
  startingMessage,
  onSelect,
}: {
  startingMessage?: string
  onSelect: (provider: OAuthProviderKind) => void
}) {
  const [selected, setSelected] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'up' || event.sequence === 'k') {
      setSelected(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || event.sequence === 'j') {
      setSelected(prev => Math.min(DEFAULT_OPTIONS.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const opt = DEFAULT_OPTIONS[selected]
      if (opt) onSelect(opt.value)
    }
  })

  return (
    <box flexDirection="column" gap={1}>
      <text>
        <strong>
          {startingMessage ??
            'Claude Code can be used with your Claude subscription or billed based on API usage through your Console account.'}
        </strong>
      </text>
      <text>Select login method:</text>
      <box flexDirection="column">
        {DEFAULT_OPTIONS.map((opt, i) => {
          const isSelected = i === selected
          return (
            <box key={opt.value} flexDirection="column" marginTop={i === 0 ? 0 : 1}>
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` ${opt.title} `}</strong>
                {'  '}
                <span fg={isSelected ? c.bg : c.dim}>\u00B7 {opt.description}</span>
              </text>
            </box>
          )
        })}
      </box>
      <text fg={c.dim}>Up/Down to move \u00B7 Enter to select</text>
    </box>
  )
}

function PlatformSetupView() {
  return (
    <box flexDirection="column" gap={1}>
      <text>
        <strong>Using 3rd-party platforms</strong>
      </text>
      <text>
        Claude Code supports Amazon Bedrock, Microsoft Foundry, and Vertex
        AI. Set the required environment variables, then restart Claude
        Code.
      </text>
      <text>
        If you are part of an enterprise organization, contact your
        administrator for setup instructions.
      </text>
      <box flexDirection="column" marginTop={1}>
        <text>
          <strong>Documentation</strong>
        </text>
        <text>
          \u00B7 Amazon Bedrock: <span fg={c.info}>https://docs.claude.com/en/docs/claude-code/amazon-bedrock</span>
        </text>
        <text>
          \u00B7 Microsoft Foundry: <span fg={c.info}>https://docs.claude.com/en/docs/claude-code/microsoft-foundry</span>
        </text>
        <text>
          \u00B7 Vertex AI: <span fg={c.info}>https://docs.claude.com/en/docs/claude-code/google-vertex-ai</span>
        </text>
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          Press <strong>Enter</strong> to go back to login options.
        </text>
      </box>
    </box>
  )
}

function ProviderFormView({
  status,
}: {
  status: Extract<ConsoleOAuthStatus, { state: 'provider_form' }>
}) {
  const providerTitle =
    status.provider === 'openai_chat_api'
      ? 'OpenAI Compatible API Setup'
      : status.provider === 'gemini_api'
        ? 'Gemini API Setup'
        : 'Anthropic Compatible Setup'

  return (
    <box flexDirection="column" gap={1}>
      <text>
        <strong>{providerTitle}</strong>
      </text>
      <box flexDirection="column" gap={1}>
        {FORM_FIELDS.map(field => {
          const active = status.activeField === field
          const value = status.values[field] ?? ''
          const isMasked = field === 'api_key'
          const display = isMasked && value
            ? value.slice(0, 8) + '\u00B7'.repeat(Math.max(0, value.length - 8))
            : value
          return (
            <box key={field} flexDirection="row">
              <text
                fg={active ? c.bg : undefined}
                bg={active ? c.textBright : undefined}
              >
                {` ${FIELD_LABELS[field]} `}
              </text>
              <text>
                {' '}
                {display ? (
                  <span fg={c.success}>{display}</span>
                ) : (
                  <span fg={c.dim}>(empty)</span>
                )}
              </text>
            </box>
          )
        })}
      </box>
      <text fg={c.dim}>
        \u2191\u2193/Tab to switch \u00B7 Enter on last field to save \u00B7 Esc to go back
      </text>
    </box>
  )
}

function WaitingForLoginView({
  status,
  forcedMethodMessage,
  onCopyUrl,
  onSubmitManualCode,
}: {
  status: Extract<ConsoleOAuthStatus, { state: 'waiting_for_login' }>
  forcedMethodMessage?: string | null
  onCopyUrl: (url: string) => void
  onSubmitManualCode: (code: string) => void
}) {
  const [code, setCode] = useState('')

  useKeyboard(event => {
    if (!status.showPastePrompt) return
    if (event.eventType === 'release') return
    const name = event.name
    if (event.sequence === 'c') {
      onCopyUrl(status.url)
      return
    }
    if (name === 'backspace') {
      setCode(prev => prev.slice(0, -1))
      return
    }
    if (name === 'return' || name === 'enter') {
      if (code) onSubmitManualCode(code)
      return
    }
    if (event.sequence && event.sequence.length === 1) {
      setCode(prev => prev + event.sequence)
    }
  })

  return (
    <box flexDirection="column" gap={1}>
      {forcedMethodMessage && (
        <text fg={c.dim}>{forcedMethodMessage}</text>
      )}
      {!status.showPastePrompt ? (
        <box flexDirection="row">
          <Spinner label="Opening browser to sign in\u2026" />
        </box>
      ) : (
        <>
          <box flexDirection="column" gap={0}>
            <text fg={c.dim}>
              Browser didn&apos;t open? Use the url below to sign in.
            </text>
            <text fg={c.info}>{status.url}</text>
            <text fg={c.dim}>Press <strong>c</strong> to copy the URL.</text>
          </box>
          <box flexDirection="row">
            <text>Paste code here if prompted &gt; </text>
            <text>{'*'.repeat(code.length)}</text>
          </box>
        </>
      )}
    </box>
  )
}

function SuccessView({
  status,
  mode,
}: {
  status: Extract<ConsoleOAuthStatus, { state: 'success' }>
  mode: 'login' | 'setup-token'
}) {
  if (mode === 'setup-token' && status.token) {
    return (
      <box flexDirection="column" gap={1}>
        <text fg={c.success}>
          \u2713 Long-lived authentication token created successfully!
        </text>
        <text>Your OAuth token (valid for 1 year):</text>
        <text fg={c.warning}>{status.token}</text>
        <text fg={c.dim}>
          Store this token securely. You won&apos;t be able to see it again.
        </text>
        <text fg={c.dim}>
          Use this token by setting: export CLAUDE_CODE_OAUTH_TOKEN=&lt;token&gt;
        </text>
      </box>
    )
  }
  return (
    <box flexDirection="column">
      {status.email && (
        <text fg={c.dim}>
          Logged in as <span fg={c.text}>{status.email}</span>
        </text>
      )}
      <text fg={c.success}>
        Login successful. Press <strong>Enter</strong> to continue\u2026
      </text>
    </box>
  )
}

function ErrorView({
  status,
}: {
  status: Extract<ConsoleOAuthStatus, { state: 'error' }>
}) {
  return (
    <box flexDirection="column" gap={1}>
      <text fg={c.error}>OAuth error: {status.message}</text>
      {status.canRetry && (
        <text fg={c.warning}>
          Press <strong>Enter</strong> to retry.
        </text>
      )}
    </box>
  )
}

export function ConsoleOAuthFlow({
  status,
  mode = 'login',
  startingMessage,
  forcedMethodMessage,
  onSelectProvider,
  onSubmitManualCode,
  onRetry,
  onCopyUrl,
  onDone,
}: Props) {
  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    if (status.state === 'success' && mode !== 'setup-token') {
      if (name === 'return' || name === 'enter') onDone()
      return
    }
    if (status.state === 'platform_setup') {
      if (name === 'return' || name === 'enter') onSelectProvider('platform')
      return
    }
    if (status.state === 'error' && status.canRetry) {
      if (name === 'return' || name === 'enter') onRetry()
      return
    }
  })

  useEffect(() => {
    if (mode === 'setup-token' && status.state === 'success') {
      const id = setTimeout(onDone, 500)
      return () => clearTimeout(id)
    }
  }, [mode, status.state, onDone])

  let body: React.ReactNode = null
  switch (status.state) {
    case 'idle':
      body = <IdleView startingMessage={startingMessage} onSelect={onSelectProvider} />
      break
    case 'platform_setup':
      body = <PlatformSetupView />
      break
    case 'provider_form':
      body = <ProviderFormView status={status} />
      break
    case 'ready_to_start':
      body = (
        <box flexDirection="row">
          <Spinner label="Starting OAuth flow\u2026" />
        </box>
      )
      break
    case 'waiting_for_login':
      body = (
        <WaitingForLoginView
          status={status}
          forcedMethodMessage={forcedMethodMessage}
          onCopyUrl={onCopyUrl}
          onSubmitManualCode={onSubmitManualCode}
        />
      )
      break
    case 'creating_api_key':
      body = (
        <box flexDirection="row">
          <Spinner label="Creating API key for Claude Code\u2026" />
        </box>
      )
      break
    case 'about_to_retry':
      body = <text fg={c.warning}>Retrying\u2026</text>
      break
    case 'success':
      body = <SuccessView status={status} mode={mode} />
      break
    case 'error':
      body = <ErrorView status={status} />
      break
  }

  return (
    <box flexDirection="column" gap={1} paddingX={1}>
      {body}
    </box>
  )
}
