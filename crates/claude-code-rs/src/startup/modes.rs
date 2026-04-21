//! Non-interactive output modes — `--output-format json` (JSONL on stdout)
//! and `-p` plain print mode. Both consume the same SDK message stream and
//! map `SdkMessage::Result::is_error` onto the process exit code.

use std::process::ExitCode;

use anyhow::Context;

use crate::engine::lifecycle::QueryEngine;
use crate::types::config::QuerySource;

/// JSON output mode (for SDK consumers — JSONL on stdout).
pub async fn run_json_mode(engine: &QueryEngine, prompt: &str) -> anyhow::Result<ExitCode> {
    use futures::StreamExt;

    let stream = engine.submit_message(prompt, QuerySource::Sdk);
    let mut stream = std::pin::pin!(stream);
    let mut exit_code = ExitCode::SUCCESS;

    while let Some(msg) = stream.next().await {
        let json = serde_json::to_string(&msg).context("failed to serialize SdkMessage to JSON")?;
        println!("{}", json);

        if let crate::engine::sdk_types::SdkMessage::Result(ref result) = msg {
            if result.is_error {
                exit_code = ExitCode::FAILURE;
            }
        }
    }

    Ok(exit_code)
}

/// Plain-text print mode (non-interactive `-p`). Emits assistant text
/// blocks as they arrive and a trailing newline at the end.
pub async fn run_print_mode(engine: &QueryEngine, prompt: &str) -> anyhow::Result<ExitCode> {
    use futures::StreamExt;

    let stream = engine.submit_message(prompt, QuerySource::Sdk);
    let mut stream = std::pin::pin!(stream);

    let mut exit_code = ExitCode::SUCCESS;

    while let Some(msg) = stream.next().await {
        match &msg {
            crate::engine::sdk_types::SdkMessage::Assistant(assistant_msg) => {
                for block in &assistant_msg.message.content {
                    if let crate::types::message::ContentBlock::Text { text } = block {
                        print!("{}", text);
                    }
                }
            }
            crate::engine::sdk_types::SdkMessage::Result(result) => {
                if result.is_error {
                    exit_code = ExitCode::FAILURE;
                }
            }
            _ => {}
        }
    }

    println!(); // trailing newline
    Ok(exit_code)
}
