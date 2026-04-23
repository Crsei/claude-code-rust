import React from 'react'
import { act } from 'react'
import { afterEach, expect, test } from 'bun:test'
import { testRender } from '@opentui/react/test-utils'
import { AppStateProvider } from '../../store/app-store.js'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { AgentDetail } from '../agents/AgentDetail.js'
import { DiffDialog } from '../diff/DiffDialog.js'
import { HelpV2 } from '../helpv2/index.js'
import { LspRecommendationMenu } from '../LspRecommendation/LspRecommendationMenu.js'
import { Settings } from '../Settings/Settings.js'
import { parseUnifiedDiff } from '../StructuredDiff/index.js'
import type { DiffFile } from '../diff/DiffFileList.js'

let renderer: Awaited<ReturnType<typeof testRender>>['renderer'] | null = null

afterEach(async () => {
  await act(async () => {
    renderer?.destroy()
  })
  renderer = null
})

async function renderFrame(node: React.ReactNode): Promise<string> {
  const setup = await testRender(node, { width: 100, height: 30 })
  renderer = setup.renderer
  await setup.renderOnce()
  return setup.captureCharFrame()
}

function withAppState(node: React.ReactNode): React.ReactNode {
  return <AppStateProvider>{node}</AppStateProvider>
}

test('recent agent detail renders cc-rust isolated paths and prompt markdown', async () => {
  const agent: AgentDefinitionEntry = {
    name: 'reviewer',
    description: 'Reviews changes',
    system_prompt: '# Review\nCheck the diff.',
    tools: ['Read'],
    source: { kind: 'project' },
  }

  const frame = await renderFrame(
    withAppState(<AgentDetail agent={agent} onBack={() => {}} />),
  )

  expect(frame).toContain('.cc-rust/agents/reviewer.md')
  expect(frame).toContain('Reviews changes')
})

test('recent help and settings overlays render visible OpenTUI frames', async () => {
  const helpFrame = await renderFrame(
    withAppState(
      <HelpV2
        onClose={() => {}}
        builtinCommands={[{ name: 'help', description: 'Show help' }]}
        version="test"
      />,
    ),
  )
  expect(helpFrame).toContain('Claude Code vtest')
  expect(helpFrame).toContain('commands')

  await act(async () => {
    renderer?.destroy()
  })
  renderer = null

  const settingsFrame = await renderFrame(
    withAppState(<Settings onClose={() => {}} defaultTab="Status" />),
  )
  expect(settingsFrame).toContain('Settings')
  expect(settingsFrame).toContain('Status')
})

test('recent diff dialog renders file list and parsed hunks', async () => {
  const diff = [
    'diff --git a/src/a.ts b/src/a.ts',
    '--- a/src/a.ts',
    '+++ b/src/a.ts',
    '@@ -1,2 +1,2 @@',
    ' old',
    '-before',
    '+after',
  ].join('\n')
  const file: DiffFile = {
    path: 'src/a.ts',
    linesAdded: 1,
    linesRemoved: 1,
  }

  const frame = await renderFrame(
    <DiffDialog
      sources={[{
        label: 'Current',
        files: [file],
        hunks: new Map([[file.path, parseUnifiedDiff(diff)]]),
        stats: { filesCount: 1, linesAdded: 1, linesRemoved: 1 },
      }]}
      onDone={() => {}}
    />,
  )

  expect(frame).toContain('Uncommitted changes')
  expect(frame).toContain('src/a.ts')
  expect(frame).toContain('+1')
  expect(frame).toContain('-1')
})

test('recent LSP recommendation menu renders keyboard-selectable choices', async () => {
  const frame = await renderFrame(
    <LspRecommendationMenu
      pluginName="typescript-language-server"
      fileExtension=".ts"
      onResponse={() => {}}
      autoDismissMs={0}
    />,
  )

  expect(frame).toContain('LSP Plugin Recommendation')
  expect(frame).toContain('Yes, install typescript-language-server')
  expect(frame).toContain('Disable all LSP recommendations')
})
