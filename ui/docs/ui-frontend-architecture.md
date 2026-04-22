# OpenTUI Lite frontend — adopted architecture

Status as of the Issue 07 landing (`tui` branch).

The active frontend lives entirely under `ui/src/` and targets OpenTUI
(`@opentui/core`, `@opentui/react`). The upstream sample tree in
`ui/examples/upstream-patterns/` stays in the repo for design-pattern
reference — **no active code imports from it**. Earlier iterations of
the migration used placeholder barrels under `ui/src/components/mcp/`,
`shell/`, `teams/`; those were removed once the actual work landed
elsewhere (see below).

```
ui/src/
├── main.tsx                     OpenTUI renderer bootstrap
├── theme.ts                     palette (`c.*`) shared across panels
├── utils.ts                     formatCost / formatTokens / uid / truncate
├── commands.ts                  slash command registry
├── keybindings.ts               shortcut config + matchers
├── ipc/
│   ├── protocol.ts              backend → frontend types (shared with Rust)
│   └── context.tsx              `useBackend()` hook
├── store/
│   ├── app-state.ts             `AppState` + action union + initial state
│   ├── app-store.tsx            reducer dispatcher + React context
│   ├── message-model.ts         `RawMessage` → `RenderItem` pipeline
│   └── reducers/                core, input, tool-activity, subsystems, teams…
├── view-model/                  normalized cross-slice types (Issue 01)
├── adapters/                    IPC → view-model mappers (Issue 01, 04)
└── components/
    ├── App.tsx                  IPC event loop + layout
    ├── InputPrompt.tsx          composer orchestrator (Issue 06)
    ├── MessageBubble.tsx        dispatcher → ./messages leaves
    ├── messages/                per-type leaves (Issue 03)
    ├── permissions/             category-aware dialog (Issue 04)
    ├── panels/                  MCP/LSP/plugin/team cards (Issue 05)
    ├── PromptInput/             composer submodules (Issue 06)
    ├── StatusLine/              built-in + custom statusline (Issue 07)
    ├── StructuredDiff/          pure hunk parser + renderer (Issue 04)
    ├── FilePathLink.tsx         OSC 8 hyperlinks (Issue 02)
    ├── OrderedList.tsx          nested ordered list (Issue 02)
    ├── TagTabs.tsx              horizontally scrolling tabs (Issue 02)
    ├── ValidationErrorsList.tsx errors grouped by file (Issue 02)
    ├── PressEnterToContinue.tsx confirm hint (Issue 02)
    ├── Spinner.tsx              braille-dot spinner
    ├── AgentTreePanel.tsx       nested running-agents tree
    ├── SubsystemStatus.tsx      frame around panels/*
    ├── TeamPanel.tsx            frame around team cards
    ├── CommandHint.tsx          slash-command + sub-mode hint (consumed
    │                            by PromptInput/SlashCommandHints)
    ├── DiffView.tsx             legacy simple diff frame
    ├── ThinkingBlock.tsx        removed (replaced by messages/ThinkingPreview)
    ├── ServerListEditor.tsx     editor table used by /mcp edit
    ├── Suggestions.tsx          follow-up suggestion chips
    └── WelcomeScreen.tsx        empty-conversation hero
```

## Data flow

```
backend (Rust IPC) ─► App.tsx message handler ─► store dispatch
                                                  │
                                                  ▼
                         AppState (AppStateProvider)
                                                  │
                                                  ▼
         ┌──────────────────────────────┬─────────────────────────┐
         ▼                              ▼                         ▼
   components/messages/*      components/panels/*        components/StatusLine/
   (consume RenderItem +      (consume SubsystemState +  (consume AppState +
   adapter helpers)           pure summarizeTeam)        custom statusline)
```

`RenderItem`s come from `store/message-model.ts` (existing pipeline);
`MessageViewModel`s and adapter helpers come from `adapters/` (added
by Issue 01) and are consumed by `messages/SystemMessage`,
`messages/ToolActivityMessage` (via `ToolStatus`),
`messages/FileEditToolPreview` (via `extractFileEditContext`), and the
`permissions/` variants (via `mapPermissionRequestToViewModel`).

## Status line

The statusline is always composed of two layers, with explicit
precedence:

1. **Built-in** (`StatusLine/BuiltinStatusLine.tsx`). Derived from the
   store every render — `cwd`, `model`, running agents, active teams,
   connected MCP, running LSP, usage tokens/cost. Always rendered
   while the conversation is active.

2. **Custom** (`StatusLine/CustomStatusLine.tsx`). Rendered below the
   built-in row when the backend has forwarded a
   `status_line_update` IPC event with a non-empty, error-free
   `lines[]` snapshot. The snapshot lives under
   `AppState.customStatusLine` and is updated by the
   `CUSTOM_STATUS_LINE_UPDATE` action. A small red pill on the
   built-in row surfaces the runner error when present.

Decision rationale: the sample tree's `StatusLine`/`BuiltinStatusLine`
pair does the same thing — Built-in is authoritative, Custom is an
overlay when the user configures `statusLine.command`. Running both in
parallel keeps the baseline reliable (no silent gap if the custom
runner exits) and lets the operator still see their own script
output.

## Composer

`InputPrompt.tsx` owns the app-wide keyboard dispatch + vim state.
Every presentational piece and every submission contract are
decomposed into `PromptInput/`:

- `ComposerBuffer` — before/cursor/after, paste-compact, transcript
  readonly.
- `QueuedSubmissions` — queued-submission preview row.
- `ModeIndicator` — reasoning/thinking elapsed tag.
- `SlashCommandHints` — autocomplete + sub-mode option selector.
- `useComposerSubmit` — submit / queue / sendCommand /
  activateCommand.
- `prompt-state.ts` — cursor split, paste predicate, busy-status
  derivation.
- `hooks.ts`, `keys.ts`, `utils.ts` — state hooks + key-event
  normalization + paste detection helpers.

## Permission flow

`App.tsx` renders `permissions/PermissionRequestDialog` as an absolute
overlay when `state.permissionRequest` is non-null. The dialog runs
the incoming request through `mapPermissionRequestToViewModel`
(Issue 01 adapter) and dispatches by `PermissionCategory` to one of:

- `BashPermissionRequest`
- `FileEditPermissionRequest` — uses `StructuredDiff` + `hunkFromEdit`
  to preview the proposed edit.
- `FileWritePermissionRequest`
- `WebFetchPermissionRequest`
- `FallbackPermissionRequest`

The shared `PermissionPromptOptions` handles keyboard navigation and
backend-provided / label-inferred hotkeys.

## Tool result rendering

`messages/ToolActivityMessage` renders the generic activity row. When
the tool name matches `isFileEditToolName`, a `FileEditToolPreview`
renders above the "Result" line, using the same `StructuredDiff`
renderer as the permission dialog.

## Operational panels

All MCP / LSP / plugin / team cards live under
`components/panels/`, consumed by `SubsystemStatus` and `TeamPanel`.
Pure helpers (`summarizeTeam`, `summarizeTeams`, `stateColor`,
`isHealthyState`) keep the title math unit-testable.

Upstream panels that can't be supported by the current protocol are
documented — with explicit blockers — in
[ui-panels-deferred.md](./ui-panels-deferred.md).

## Testing surface

Every new module ships with a `bun:test` file under `__tests__/`
covering the pure helpers it exports. As of Issue 07:

- `adapters/__tests__/` — content-blocks, messages, permissions,
  tool-input, tool-status, file-edit, system-level.
- `components/__tests__/` — file-path-link, string-width, tag-tabs,
  validation-errors-list, paste-display, server-list-editor.
- `components/StructuredDiff/__tests__/` — hunks parser + builder +
  gutter math.
- `components/permissions/__tests__/` — hotkey resolution.
- `components/panels/__tests__/` — state colours + team summary.
- `components/PromptInput/__tests__/` — prompt-state helpers.
- `components/StatusLine/__tests__/` — custom-line gating + counts +
  cwd shortener.
