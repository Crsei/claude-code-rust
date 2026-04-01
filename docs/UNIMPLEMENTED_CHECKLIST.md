# Rust Rewrite Unimplemented Checklist

> Generated on 2026-04-01 by comparing [`TOOLS_AND_COMMANDS.md`](../../TOOLS_AND_COMMANDS.md)
> with [`REWRITE_PLAN.md`](./REWRITE_PLAN.md).

## Notes

- Original public command names from `TOOLS_AND_COMMANDS.md` are used in this checklist.
- Naming alignment:
  - `sandbox-toggle` in the plan maps to `/sandbox`
  - `thinkback` in the plan maps to `/think-back`
  - `bridge` in the plan maps to `/remote-control`
  - `remote-setup` in the plan maps to `/web-setup`
  - `terminalSetup` in the plan maps to `/terminal-setup`
- Status intent:
  - "Planned" means listed in `REWRITE_PLAN.md` but not complete yet
  - "Unmapped" means present in `TOOLS_AND_COMMANDS.md` but not clearly covered by `REWRITE_PLAN.md`
  - "Deferred" means the plan explicitly says not to implement it for now

## Commands: Planned But Not Complete

- [ ] `/config` complete implementation; currently scaffold only
- [ ] `/add-dir`
- [ ] `/agents`
- [ ] `/color`
- [ ] `/copy`
- [ ] `/doctor`
- [ ] `/feedback`
- [ ] `/ide`
- [ ] `/init`
- [ ] `/keybindings`
- [ ] `/mcp`
- [ ] `/plugin`
- [ ] `/privacy-settings`
- [ ] `/rewind`
- [ ] `/sandbox`
- [ ] `/skills`
- [ ] `/status`
- [ ] `/tag`
- [ ] `/tasks`
- [ ] `/theme`
- [ ] `/think-back`
- [ ] `/upgrade`
- [ ] `/vim`
- [ ] `/voice`

## Commands: Unmapped In Rewrite Plan

- [ ] `/commit-push-pr`
- [ ] `/security-review`
- [ ] `/pr-comments`
- [ ] `/advisor`
- [ ] `/btw`
- [ ] `/insights`
- [ ] `/install-github-app`
- [ ] `/install-slack-app`
- [ ] `/output-style`
- [ ] `/extra-usage`
- [ ] `/passes`
- [ ] `/reload-plugins`
- [ ] `/statusline`
- [ ] `/ultrareview`
- [ ] `/assistant`
- [ ] `/brief`
- [ ] `/proactive`
- [ ] `/force-snip`
- [ ] `/fork`
- [ ] `/peers`
- [ ] `/buddy`
- [ ] `/subscribe-pr`
- [ ] `/torch`
- [ ] `/ultraplan`
- [ ] `/workflows`
- [ ] `/heapdump`
- [ ] `/rate-limit-options`
- [ ] `/thinkback-play`

## Commands: Internal Or Ant-Only Unmapped

- [ ] `/agents-platform`
- [ ] `/ant-trace`
- [ ] `/autofix-pr`
- [ ] `/backfill-sessions`
- [ ] `/break-cache`
- [ ] `/bridge-kick`
- [ ] `/bughunter`
- [ ] `/ctx-viz`
- [ ] `/debug-tool-call`
- [ ] `/env`
- [ ] `/good-claude`
- [ ] `/init-verifiers`
- [ ] `/issue`
- [ ] `/mock-limits`
- [ ] `/oauth-refresh`
- [ ] `/onboarding`
- [ ] `/perf-issue`
- [ ] `/reset-limits`
- [ ] `/share`
- [ ] `/summary`
- [ ] `/teleport`

## Commands: Explicitly Deferred

- [ ] `/remote-control`
- [ ] `/web-setup`
- [ ] `/chrome`
- [ ] `/desktop`
- [ ] `/mobile`
- [ ] `/remote-env`
- [ ] `/release-notes`
- [ ] `/stickers`
- [ ] `/terminal-setup`
- [ ] `/usage`

## Tools: Planned But Not Complete

- [ ] `EnterPlanMode`
- [ ] `ExitPlanMode`
- [ ] `EnterWorktree`
- [ ] `ExitWorktree`
- [ ] `Skill`
- [ ] `WebFetch`
- [ ] `WebSearch`
- [ ] `SendMessage`
- [ ] `LSP`
- [ ] `PowerShell`
- [ ] `Sleep`
- [ ] `SendUserMessage (Brief)`
- [ ] `Config`
- [ ] `RemoteTrigger`
- [ ] `CronCreate`
- [ ] `CronDelete`
- [ ] `CronList`
- [ ] `REPL`

## Tools: Unmapped In Rewrite Plan

- [ ] `TodoWrite`
- [ ] `McpAuthTool`
- [ ] `TeamCreate`
- [ ] `TeamDelete`
- [ ] `Monitor`
- [ ] `WebBrowser`
- [ ] `SnipTool`
- [ ] `ListPeers`
- [ ] `Workflow`
- [ ] `TerminalCapture`
- [ ] `CtxInspect`
- [ ] `OverflowTest`
- [ ] `StructuredOutput`
- [ ] `VerifyPlanExecution`
- [ ] `Tungsten`
- [ ] `SuggestBackgroundPR`
- [ ] `SendUserFile`
- [ ] `PushNotification`
- [ ] `SubscribePR`

## Core Modules And Services: Planned But Not Complete

- [ ] `api/providers.rs`: AWS Bedrock provider implementation
- [ ] `api/providers.rs`: GCP Vertex provider implementation
- [ ] `auth/token.rs`: OAuth token persistence implementation
- [ ] `auth/mod.rs`: OAuth login/refresh/logout implementation
- [ ] `analytics/mod.rs`: network telemetry sending
- [ ] `remote/session.rs`: remote session implementation
- [ ] `utils/messages.rs`: message formatting, truncation, counting
- [ ] `ui/keybindings.rs`: keybinding registration and custom bindings
- [ ] `ui/vim.rs`: Vim mode support
- [ ] `session/migrations.rs`: session data migrations
- [ ] `session/` memdir-backed memory read/write
- [ ] `services/lsp/`: LSP service layer
- [ ] `services/plugins/` and `plugins/`: plugin loading and management
- [ ] `skills/`: skill discovery and execution
- [ ] `services/SessionMemory/`: session memory service
- [ ] `services/PromptSuggestion/`: prompt suggestion/input completion
- [ ] `services/toolUseSummary/`: tool-use summary/statistics

## Directories And Platforms: Explicitly Deferred

- [ ] `bridge/`
- [ ] `cli/transports/`
- [ ] `coordinator/`
- [ ] `server/`
- [ ] `remote/` extended CCR/cloud-container support
- [ ] `services/remoteManagedSettings/`
- [ ] `services/analytics/` remote telemetry pipeline
- [ ] desktop integration
- [ ] mobile integration
