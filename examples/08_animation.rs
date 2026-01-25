//! Example 08: Simple Animation Loop
//!
//! Demonstrates:
//! - A basic render loop with timing
//! - Moving a character across the screen
//! - Frame pacing with sleep

use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{OptimizedBuffer, Renderer, Rgba, Style};
use std::io::{self, Read};
use std::time::{Duration, Instant};

fn text_len_u32(text: &str) -> u32 {
    u32::try_from(text.len()).unwrap_or(u32::MAX)
}

fn center_x(width: u32, text: &str) -> u32 {
    width.saturating_sub(text_len_u32(text)) / 2
}

fn draw_text(
    buffer: &mut OptimizedBuffer,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    text: &str,
    style: Style,
) {
    if y < height && x < width {
        buffer.draw_text(x, y, text, style);
    }
}

fn step_from_dt(speed: u32, dt: Duration) -> u32 {
    let ms = u32::try_from(dt.as_millis().min(u128::from(u32::MAX))).unwrap_or(u32::MAX);
    let step = speed.saturating_mul(ms) / 1000;
    step.max(1)
}

fn main() -> io::Result<()> {
    let (term_w, term_h) = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(u32::from(term_w), u32::from(term_h))?;
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let baseline = height.saturating_sub(6).max(6);

    let mut x = 2u32;
    let mut dir = 1i32;
    let speed = 25u32; // cells per second

    let mut last = Instant::now();
    let start = Instant::now();

    // Run for ~6 seconds or until Ctrl+C (depends on terminal sending).
    while start.elapsed() < Duration::from_secs(6) {
        let now = Instant::now();
        let dt = now.saturating_duration_since(last);
        last = now;

        let step = step_from_dt(speed, dt);
        if dir > 0 {
            x = x.saturating_add(step);
            if x >= width.saturating_sub(3) {
                dir = -1;
                x = width.saturating_sub(3);
            }
        } else {
            x = x.saturating_sub(step);
            if x <= 2 {
                dir = 1;
                x = 2;
            }
        }

        {
            let buffer = renderer.buffer();
            buffer.clear(Rgba::from_hex("#0f111a").expect("valid"));

            let title = "Simple Animation";
            let title_x = center_x(width, title);
            draw_text(
                buffer,
                width,
                height,
                title_x,
                1,
                title,
                Style::fg(Rgba::from_hex("#4cd137").expect("valid")).with_bold(),
            );

            draw_text(
                buffer,
                width,
                height,
                2,
                3,
                "Rendering a moving sprite at ~25 cells/sec",
                Style::fg(Rgba::WHITE),
            );

            draw_text(
                buffer,
                width,
                height,
                x,
                baseline,
                "●",
                Style::fg(Rgba::from_hex("#fbc531").expect("valid")).with_bold(),
            );
        }

        renderer.present()?;
        std::thread::sleep(Duration::from_millis(16));
    }

    // Wait for a key before exiting to show final frame.
    let _ = io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
