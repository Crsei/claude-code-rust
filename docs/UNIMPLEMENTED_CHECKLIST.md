# Rust Rewrite Unimplemented Checklist

> Updated on 2026-04-02 by comparing [`TOOLS_AND_COMMANDS.md`](../../TOOLS_AND_COMMANDS.md)
> with [`REWRITE_PLAN.md`](./REWRITE_PLAN.md).

## Notes

- Original public command names from `TOOLS_AND_COMMANDS.md` are used in this checklist.
- Status intent:
  - "Planned" means listed in `REWRITE_PLAN.md` but not complete yet
  - "Unmapped" means present in `TOOLS_AND_COMMANDS.md` but not clearly covered by `REWRITE_PLAN.md`
  - "Deferred" means the plan explicitly says not to implement it for now

## Commands: Implemented (74 total)

- [x] `/help`, `/clear`, `/compact`, `/config`, `/diff`, `/exit`, `/version`
- [x] `/model`, `/cost`, `/session`, `/resume`, `/files`, `/context`
- [x] `/permissions`, `/hooks`, `/login`, `/logout`
- [x] `/commit`, `/review`, `/branch`, `/export`, `/rename`, `/stats`
- [x] `/effort`, `/fast`, `/memory`, `/plan`
- [x] `/add-dir`, `/init`, `/copy`, `/doctor`, `/tasks`, `/status`
- [x] `/theme`, `/color`, `/rewind`, `/skills`, `/mcp`, `/plugin`
- [x] `/keybindings`, `/feedback`, `/tag`, `/think-back`, `/sandbox`
- [x] `/force-snip`, `/fork`, `/output-style`
- [x] `/agents`, `/upgrade`, `/ide`, `/privacy-settings`
- [x] `/security-review`, `/pr-comments`, `/commit-push-pr`
- [x] `/brief`, `/proactive`, `/vim`
- [x] `/voice`, `/advisor`, `/btw`, `/insights`, `/passes`, `/reload-plugins`
- [x] `/statusline`, `/ultrareview`, `/ultraplan`, `/thinkback-play`
- [x] `/install-github-app`, `/install-slack-app`
- [x] `/workflows`, `/subscribe-pr`, `/peers`, `/buddy`, `/torch`

## Commands: Planned But Not Complete

- [ ] `/extra-usage` — model-specific billing (deferred)
- [ ] `/rate-limit-options` — model-specific rate limits (deferred)

## Commands: Internal Or Ant-Only (Not Implementing)

- `/agents-platform`, `/ant-trace`, `/autofix-pr`, `/backfill-sessions`
- `/break-cache`, `/bridge-kick`, `/bughunter`, `/ctx-viz`
- `/debug-tool-call`, `/env`, `/good-claude`, `/init-verifiers`
- `/issue`, `/mock-limits`, `/oauth-refresh`, `/onboarding`
- `/perf-issue`, `/reset-limits`, `/share`, `/summary`
- `/teleport`, `/heapdump`

## Commands: Explicitly Deferred

- `/remote-control`, `/web-setup`, `/chrome`, `/desktop`, `/mobile`
- `/remote-env`, `/release-notes`, `/stickers`, `/terminal-setup`, `/usage`

## Tools: Implemented (30 total)

- [x] `Bash`, `Read`, `Write`, `Edit`, `Glob`, `Grep`
- [x] `NotebookEdit`, `AskUserQuestion`, `ToolSearch`
- [x] `Agent`, `EnterPlanMode`, `ExitPlanMode`
- [x] `EnterWorktree`, `ExitWorktree`
- [x] `Skill`, `WebFetch`, `WebSearch`, `LSP`
- [x] `TaskCreate`, `TaskGet`, `TaskUpdate`, `TaskList`, `TaskStop`, `TaskOutput`
- [x] `TodoWrite`, `Snip`, `Sleep`
- [x] `TeamCreate`, `TeamDelete`, `SendMessage`

## Tools: Planned But Not Complete

- [ ] `PowerShell` — Windows shell (medium priority)
- [ ] `Config` — Runtime settings modification (low priority)
- [ ] `REPL` — Embedded REPL (low priority)
- [ ] `StructuredOutput` — Structured output format (low priority)
- [ ] `SendUserMessage (Brief)` — Brief output (low priority)

## Tools: Explicitly Deferred (Network/Remote)

- `RemoteTrigger`, `CronCreate`, `CronDelete`, `CronList`
- `WebBrowser`, `McpAuthTool`, `Monitor`, `ListPeers`
- `Workflow`, `TerminalCapture`, `SubscribePR`, `PushNotification`
- `SendUserFile`, `SuggestBackgroundPR`

## Tools: Internal Only (Not Implementing)

- `CtxInspect`, `OverflowTest`, `VerifyPlanExecution`, `Tungsten`

## Core Modules And Services

### Completed

- [x] `api/client.rs`: Anthropic Direct provider (fully implemented)
- [x] `api/google_provider.rs`: Google Gemini provider (fully implemented)
- [x] `api/openai_compat.rs`: OpenAI-compatible provider (fully implemented)
- [x] `auth/api_key.rs`: API key handling (fully implemented)
- [x] `auth/token.rs`: External token handling (fully implemented)
- [x] `ui/keybindings.rs`: Keybinding system (425 lines, 8 tests)
- [x] `ui/vim.rs`: Vim mode (847 lines, 22 tests)
- [x] `ui/tui.rs`: Full-screen TUI (async event loop + QueryEngine)
- [x] `session/migrations.rs`: Session data migrations (v1→v2→v3)
- [x] `session/memdir.rs`: Memory directory CRUD
- [x] `skills/loader.rs`: Skill discovery and loading
- [x] `skills/bundled.rs`: 5 bundled skills
- [x] `plugins/loader.rs`: Plugin loading
- [x] `lsp_service/mod.rs`: LSP service configuration
- [x] `permissions/path_validation.rs`: Path validation and traversal protection
- [x] `config/validation.rs`: Settings validation framework

### Planned But Not Complete

- [ ] `api/providers.rs`: AWS Bedrock provider implementation
- [ ] `api/providers.rs`: GCP Vertex provider implementation
- [ ] `auth/mod.rs`: OAuth login/refresh/logout implementation

### Explicitly Deferred

- [ ] `bridge/` — Remote control bridging
- [ ] `cli/transports/` — SSE, WebSocket, Worker transports
- [ ] `coordinator/` — Multi-agent coordination mode
- [ ] `server/` — Server mode
- [ ] `remote/` — Cloud container (CCR) support
- [ ] `services/remoteManagedSettings/` — MDM + sync
- [ ] `services/analytics/` — Remote telemetry pipeline
- [ ] Desktop integration
- [ ] Mobile integration
