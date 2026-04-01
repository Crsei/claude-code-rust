//! `/fast` command — toggle fast mode.
//!
//! Fast mode uses the same model (Opus 4.6) with faster output via
//! `speed: "fast"` API parameter + `fast-mode-2026-02-01` beta header.
//!
//! Current status: interface only. Full implementation requires:
//! - API request: pass `speed` param + beta header
//! - Org-level availability check (`/api/claude_code_penguin_mode`)
//! - Cooldown state machine (rate_limit / overloaded → resetAt)
//! - Model validation (only Opus 4.6 supports fast mode)
//! - Beta header latch (sticky per session, cleared on /clear and /compact)

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct FastHandler;

#[async_trait]
impl CommandHandler for FastHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        // TODO: implement fast mode toggle
        // When implemented:
        //   1. Check isFastModeAvailable (org status, model support, env var)
        //   2. Toggle ctx.app_state.fast_mode
        //   3. If enabling and current model != Opus 4.6, auto-switch model
        //   4. Persist to settings.json
        //   5. Query loop reads fast_mode → sets speed="fast" + beta header

        Ok(CommandResult::Output(
            "Fast mode is not yet implemented.\n\n\
             When available, fast mode will use the same model (Opus 4.6)\n\
             with faster output generation via the `speed` API parameter."
                .to_string(),
        ))
    }
}
