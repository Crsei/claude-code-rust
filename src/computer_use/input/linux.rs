//! Linux input simulation via xdotool (X11) or ydotool (Wayland).

use super::{CursorPosition, InputAction, MouseButton};

pub async fn execute(action: InputAction) -> anyhow::Result<String> {
    match action {
        InputAction::Click { x, y, button } => {
            let btn_num = match button {
                MouseButton::Left => 1,
                MouseButton::Middle => 2,
                MouseButton::Right => 3,
            };
            let btn_name = match button {
                MouseButton::Left => "left",
                MouseButton::Right => "right",
                MouseButton::Middle => "middle",
            };
            run_xdotool(&[
                "mousemove",
                &x.to_string(),
                &y.to_string(),
                "click",
                &btn_num.to_string(),
            ])
            .await?;
            Ok(format!("{} click at ({}, {})", btn_name, x, y))
        }
        InputAction::DoubleClick { x, y } => {
            run_xdotool(&[
                "mousemove",
                &x.to_string(),
                &y.to_string(),
                "click",
                "--repeat",
                "2",
                "1",
            ])
            .await?;
            Ok(format!("double click at ({}, {})", x, y))
        }
        InputAction::TypeText { ref text } => {
            run_xdotool(&["type", "--clearmodifiers", text]).await?;
            Ok("typed text".to_string())
        }
        InputAction::KeyPress { ref key } => {
            // xdotool uses format like "ctrl+c", "Return", "alt+Tab"
            let xdotool_key = map_key_to_xdotool(key);
            run_xdotool(&["key", "--clearmodifiers", &xdotool_key]).await?;
            Ok(format!("pressed key: {}", key))
        }
        InputAction::Scroll { x, y, amount } => {
            // Move cursor first
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()]).await?;
            // xdotool button 4 = scroll up, button 5 = scroll down
            let (button, count) = if amount > 0 {
                ("5", amount)
            } else {
                ("4", -amount)
            };
            for _ in 0..count {
                run_xdotool(&["click", button]).await?;
            }
            Ok(format!("scrolled {} at ({}, {})", amount, x, y))
        }
        InputAction::MouseMove { x, y } => {
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()]).await?;
            Ok(format!("moved mouse to ({}, {})", x, y))
        }
    }
}

pub async fn cursor_position() -> anyhow::Result<CursorPosition> {
    let output = run_xdotool(&["getmouselocation"]).await?;
    // Output format: "x:123 y:456 screen:0 window:12345"
    let mut x = 0i32;
    let mut y = 0i32;
    for part in output.split_whitespace() {
        if let Some(val) = part.strip_prefix("x:") {
            x = val.parse().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("y:") {
            y = val.parse().unwrap_or(0);
        }
    }
    Ok(CursorPosition { x, y })
}

async fn run_xdotool(args: &[&str]) -> anyhow::Result<String> {
    let output = tokio::process::Command::new("xdotool")
        .args(args)
        .output()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "xdotool not available: {}. Install with: sudo apt install xdotool",
                e
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("xdotool failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn map_key_to_xdotool(key: &str) -> String {
    // xdotool uses X11 keysym names and "+" for combos
    // Most common keys map directly
    let parts: Vec<&str> = key.split('+').collect();
    parts
        .iter()
        .map(|p| match p.to_lowercase().as_str() {
            "ctrl" | "control" => "ctrl",
            "alt" => "alt",
            "shift" => "shift",
            "super" | "cmd" | "command" | "meta" => "super",
            "return" | "enter" => "Return",
            "escape" | "esc" => "Escape",
            "tab" => "Tab",
            "backspace" | "back" => "BackSpace",
            "delete" | "del" => "Delete",
            "space" => "space",
            "up" => "Up",
            "down" => "Down",
            "left" => "Left",
            "right" => "Right",
            "home" => "Home",
            "end" => "End",
            "pageup" | "page_up" => "Prior",
            "pagedown" | "page_down" => "Next",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}
