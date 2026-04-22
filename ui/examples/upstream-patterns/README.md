# Upstream Pattern Sample

This directory holds the relocated upstream-style UI sample tree that was previously mixed into the active `ui/src/**` source tree.

Intent:
- preserve the imported file structure for design-pattern reference
- keep the active OpenTUI Lite implementation focused on `ui/src/main.tsx -> ui/src/components/App.tsx`
- avoid treating the upstream Ink/runtime-heavy files as current production entrypoints

Important:
- paths under this directory mirror the original relative layout under `ui/`
- this sample tree is for reference and staged migration work
- it is not expected to compile or run independently as-is
- high-confidence stub / empty / duplicate artifacts were intentionally excluded during relocation

See [docs/issues/2026-04-21-frontend-refactor-notes.md](/F:/AIclassmanager/cc/rust/docs/issues/2026-04-21-frontend-refactor-notes.md) for the migration notes, file classes, and refactor constraints.
