//! cc-lsp-service — LSP client service (Phase 6 scaffold).
//!
//! Issue #75 (`[workspace-split] Phase 6`): target destination for
//! `crates/claude-code-rs/src/lsp_service/` plus the `tools/lsp.rs` tool
//! wrapper. Moving the tool into this crate resolves the
//! `tools::lsp <-> lsp_service` cycle: after the move, the LSP shared types
//! (HoverInfo, SymbolInfo, SourceLocation) live in one place.
