use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
/// Export a pre-rendered transcript body to a temp file and open it in
/// `$VISUAL` / `$EDITOR`. Returns the path on success. The TUI exits
/// alternate screen while the editor runs so the user can scroll freely,
/// and re-enters before returning to the main loop.
///
/// Falls back to "just write the file" when no editor env var is set.
pub(super) async fn export_to_editor(body: &str) -> anyhow::Result<std::path::PathBuf> {
    use std::io::Write as _;
    let mut path = std::env::temp_dir();
    let stem = format!(
        "cc-rust-transcript-{}.md",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    path.push(stem);
    {
        let mut f = std::fs::File::create(&path)?;
        f.write_all(body.as_bytes())?;
    }

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .ok();
    if let Some(ed) = editor.filter(|s| !s.trim().is_empty()) {
        // Leave the alternate screen so the editor can paint over a real
        // terminal. Re-entering on the way out is handled by the caller
        // via the dirty flag + next render.
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, cursor::Show);
        let _ = terminal::disable_raw_mode();

        let parts = shell_words::split(&ed)
            .map_err(|e| anyhow::anyhow!("could not parse editor command '{}': {}", ed, e))?;
        let (program, args) = parts
            .split_first()
            .ok_or_else(|| anyhow::anyhow!("editor command is empty"))?;

        let status = tokio::process::Command::new(program)
            .args(args)
            .arg(&path)
            .status()
            .await;

        // Always re-arm the terminal, even on editor failure.
        let _ = terminal::enable_raw_mode();
        let _ = execute!(std::io::stdout(), EnterAlternateScreen, cursor::Hide);

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                return Err(anyhow::anyhow!(
                    "{} exited with status {}",
                    ed,
                    s.code().unwrap_or(-1)
                ))
            }
            Err(e) => return Err(anyhow::anyhow!("could not launch '{}': {}", ed, e)),
        }
    }
    Ok(path)
}
