// Phase 3: Permission system
//
// Permission modes: Default (ask), Auto (auto-approve with classifier), Bypass (skip all), Plan (read-only)
// Rule matching: allow/deny/ask rules grouped by source
// Dangerous command detection: rm -rf, git push --force, etc.

pub mod rules;
pub mod dangerous;
