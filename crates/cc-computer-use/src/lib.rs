//! Desktop-control primitives used by the Computer Use tools.
//!
//! **Partial extraction** — Phase 3 (issue #72) moved the platform-specific
//! screenshot and input submodules here. The `detection`, `setup`, and
//! `tools` wrappers stay in the root crate because they implement the `Tool`
//! trait, which still lives there (unblocked by Phase 5 cycle-break).

pub mod input;
pub mod screenshot;
