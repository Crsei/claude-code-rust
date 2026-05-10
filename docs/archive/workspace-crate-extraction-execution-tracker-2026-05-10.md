# Workspace Crate Extraction Execution Tracker

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Runner: `scripts/run-workspace-crate-extraction-omx.ps1`

This file is the durable execution ledger for the workspace crate extraction
lanes. Execution agents must append to it after green batches, review gates, and
final lane closeout.

## Required Batch Entry Template

```markdown
## Batch NN - <lane/task label>

Status: PASS | WARNING | BLOCKER | ERROR
Commit: <hash or not committed>
Tasks:
- <task line>

Implemented effects:
- <files moved, APIs added, shims removed>

Defects and divergences:
- <test failure, behavior drift, diagnostic issue, or none>

Follow-ups:
- <owner, exact next action, risk if deferred>

Verification:
- <command>: <pass/fail/blocked>

File-size/refactor findings:
- <path, line count, action taken>

Dependency graph notes:
- <new edges, removed edges, cycle checks>

Error visibility notes:
- <explicit errors added or preserved, redundant defensive layers removed>
```

## Lane Closeout Requirements

For each lane, add:

- implemented effects;
- defects and known divergences;
- follow-up items with owners;
- verification evidence;
- line-size/refactor guard result;
- dependency graph result;
- remaining risks and whether they block the next lane.

## Current Status

No extraction execution has started from this tracker. The current commit adds
the planning and runner control surface only.
