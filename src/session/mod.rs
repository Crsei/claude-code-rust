// Phase 6: Session persistence
//
// Session storage: save/load conversation state to ~/.claude/sessions/
// Transcript recording: append-friendly write for audit trail
// Session resume: find and restore the most recent session

pub mod storage;
pub mod transcript;
pub mod resume;
