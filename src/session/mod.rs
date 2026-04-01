// Phase 6: Session persistence
//
// Session storage: save/load conversation state to ~/.cc-rust/sessions/
// Transcript recording: append-friendly write for audit trail
// Session resume: find and restore the most recent session
// Migrations: session data format versioning and migration
// Memdir: CLAUDE.md-based memory system

pub mod storage;
pub mod transcript;
pub mod resume;
pub mod migrations;
pub mod memdir;
