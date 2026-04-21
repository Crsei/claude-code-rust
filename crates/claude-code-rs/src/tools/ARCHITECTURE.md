# `src/tools/` Architecture

This document defines how the `src/tools/` directory is organized and where new
tools should live. The goal is to keep `src/tools/` from becoming a flat
"feature dumping ground" as more capabilities are added.

## Layout

```
src/tools/
├── mod.rs                — Module index only. No tool logic.
├── ARCHITECTURE.md       — This file.
├── registry.rs           — Aggregates sub-domain tools + plugin tools.
│
├── fs/                   — Filesystem: read, write, search files.
├── exec/                 — Execution: subprocesses, REPL, sleep.
├── agent/                — Agent / dispatch / sub-agent worktree.
├── web_search/           — Web search providers + tool.
│
├── execution/            — (Infrastructure) tool-execution pipeline.
├── hooks/                — (Infrastructure) pre/post tool hooks.
│
└── <single tool>.rs      — Ungrouped tools awaiting a second sibling.
```

### Sub-domains

Each sub-domain is a directory containing:

- `mod.rs` — declares child modules and exposes a `tools() -> Tools`
  aggregator returning every tool that sub-domain owns.
- One file per tool (or small cluster of tightly-coupled tools).

`registry.rs` invokes each sub-domain's `tools()` and concatenates the results.
It does **not** reach into individual tool files, which means adding a new tool
inside an existing sub-domain only requires editing that sub-domain's
`mod.rs` — `registry.rs` stays untouched.

Current sub-domains:

| Directory        | Responsibility                                                    |
|------------------|-------------------------------------------------------------------|
| `fs/`            | Read / write / search the local filesystem.                       |
| `exec/`          | Spawn subprocesses, run REPL code, control time (sleep).          |
| `agent/`         | Dispatch sub-agents, manage per-agent worktrees.                  |
| `web_search/`    | Web search tool + provider adapters.                              |

### Infrastructure modules

`execution/` and `hooks/` are *not* tools. They are shared machinery used by
the registry and the query loop to run tool calls through the permission
pipeline and hook system. They live alongside the tool sub-domains because
their only consumers are tool-related code.

### Ungrouped single-tool files

A handful of tools still live as top-level `<name>.rs` files: `ask_user`,
`skill`, `lsp`, `tasks`, `worktree`, `web_fetch`, `plan_mode`, `send_message`,
`send_user_message`, `config_tool`, `structured_output`, `system_status`,
`brief`, `background_agents`, `orchestration`.

These are acceptable **as long as they remain a single file with a single,
clear responsibility**. As soon as a second adjacent tool appears, promote the
pair into a proper sub-domain instead of stacking more files at the top level.

## Placement rules for new tools

When adding a tool, answer these questions in order:

1. **Does it belong to an existing sub-domain?**
   If yes, add the file under that directory and add its constructor to the
   sub-domain's `tools()` function. Do not touch `registry.rs`.

2. **Does it belong with an existing top-level single-tool file?**
   If the new tool is clearly adjacent to an ungrouped file (shares
   concurrency semantics, permission model, or data sources), promote the pair
   into a new sub-domain: create `src/tools/<domain>/`, move both files in,
   give it a `mod.rs` with a `tools()` aggregator, and call that aggregator
   from `registry.rs`.

3. **Is it genuinely standalone?**
   Add it as a top-level single file. Register its constructor in
   `registry.rs`'s single-tool list. Revisit rule 2 next time a related tool
   appears.

### What *not* to do

- **Do not** add a new top-level file when an existing sub-domain already owns
  the responsibility. If the tool reads files, it goes in `fs/` — not at the
  top level.
- **Do not** import individual tool types in `registry.rs` for tools that
  belong to a sub-domain. The sub-domain's `tools()` aggregator is the single
  entry point.
- **Do not** introduce circular sub-domain imports. If `fs/` needs to call
  into `exec/`, route the call through a shared crate module (e.g. `utils/`
  or `permissions/`) instead of importing across sub-domains.
- **Do not** grow a single file past ~800 lines without splitting. Large tool
  files (`file_read.rs`, `bash.rs`, `lsp.rs`) are candidates for future
  internal splits — e.g. separating schema, validation, execution, and
  rendering into sibling files under a sub-domain directory for that tool.

## Candidate next steps

The current layout groups the two most obvious clusters (`fs/`, `exec/`).
Follow-up refactors, in rough priority order:

1. Promote `tasks.rs` + `send_message.rs` + `send_user_message.rs` +
   `background_agents.rs` + `orchestration.rs` into a `coord/` sub-domain
   (agent coordination, messaging, task queue).
2. Pair `web_fetch.rs` with `web_search/` under a `web/` sub-domain.
3. Pair `worktree.rs` with `plan_mode.rs` (workspace-control tools) under a
   `workspace/` sub-domain.
4. Group `ask_user.rs`, `brief.rs`, `structured_output.rs`,
   `send_user_message.rs`, `system_status.rs`, `config_tool.rs` under a
   `meta/` sub-domain (user-facing UX and observability tools).
5. Split large single files (`file_read.rs`, `bash.rs`, `lsp.rs`) by concern
   (schema / validation / execution / rendering) when they cross ~800 lines.

Each step should preserve the invariant that `registry.rs` only knows about
sub-domain aggregators plus a shrinking list of single tools.
