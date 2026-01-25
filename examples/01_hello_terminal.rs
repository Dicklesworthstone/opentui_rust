//! Example 01: Basic Terminal Rendering
//!
//! This example demonstrates the fundamental `OpenTUI` rendering flow:
//! 1. Create a Renderer (alternate screen, cursor hidden)
//! 2. Draw to the back buffer
//! 3. Present the frame (diff-based output)
//! 4. Wait for a key to exit
//! 5. Drop restores terminal state automatically

use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{Renderer, Rgba, Style};
use std::io::{self, Read};

fn main() -> io::Result<()> {
    // Use real terminal size when available, fall back to 80x24.
    let (width, height) = terminal_size().unwrap_or((80, 24));

    // Create renderer (enters alternate screen, hides cursor, enables mouse).
    let mut renderer = Renderer::new(u32::from(width), u32::from(height))?;

    // Enable raw mode so we can read a single keypress without Enter.
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let center_y = height / 2;
    let title_y = center_y.saturating_sub(1);
    let subtitle_y = center_y.saturating_add(1);

    {
        let buffer = renderer.buffer();
        buffer.clear(Rgba::from_hex("#1a1a2e").expect("valid color"));

        let title = "Welcome to OpenTUI!";
        let title_len = u32::try_from(title.len()).unwrap_or(u32::MAX);
        let title_x = width.saturating_sub(title_len) / 2;
        if title_y < height {
            buffer.draw_text(
                title_x,
                title_y,
                title,
                Style::fg(Rgba::from_hex("#00ff88").expect("valid color")).with_bold(),
            );
        }

        let subtitle = "Press any key to exit...";
        let subtitle_len = u32::try_from(subtitle.len()).unwrap_or(u32::MAX);
        let subtitle_x = width.saturating_sub(subtitle_len) / 2;
        if subtitle_y < height {
            buffer.draw_text(
                subtitle_x,
                subtitle_y,
                subtitle,
                Style::fg(Rgba::from_hex("#888888").expect("valid color")),
            );
        }
    }

    // Present the frame (writes diff to terminal).
    renderer.present()?;

    // Wait for a single key press, then exit.
    let _ = io::stdin().read(&mut [0u8; 1])?;

    Ok(())
}
