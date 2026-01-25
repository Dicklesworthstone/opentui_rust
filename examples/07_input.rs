//! Example 07: Keyboard and Mouse Input
//!
//! Demonstrates:
//! - Parsing keyboard input with `InputParser`
//! - Handling mouse press/release/scroll events
//! - Rendering a small status panel with the last event

use opentui::input::{Event, InputParser, KeyCode};
use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{OptimizedBuffer, Renderer, Rgba, Style};
use std::io::{self, Read};

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

fn main() -> io::Result<()> {
    let (term_w, term_h) = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(u32::from(term_w), u32::from(term_h))?;
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let mut parser = InputParser::new();
    let mut stdin = io::stdin();
    let mut buf = [0u8; 64];

    let mut last_event = String::from("<none>");
    let mut status = String::from("Press q to exit. Click or scroll to see events.");

    loop {
        {
            let buffer = renderer.buffer();
            buffer.clear(Rgba::from_hex("#0f111a").expect("valid"));

            let title = "Input Demo";
            let title_x = center_x(width, title);
            draw_text(
                buffer,
                width,
                height,
                title_x,
                1,
                title,
                Style::fg(Rgba::from_hex("#74b9ff").expect("valid")).with_bold(),
            );

            draw_text(buffer, width, height, 2, 3, &status, Style::fg(Rgba::WHITE));

            draw_text(
                buffer,
                width,
                height,
                2,
                5,
                "Last event:",
                Style::fg(Rgba::from_hex("#ffeaa7").expect("valid")).with_bold(),
            );
            draw_text(
                buffer,
                width,
                height,
                2,
                6,
                &last_event,
                Style::fg(Rgba::WHITE),
            );
        }

        renderer.present()?;

        let n = stdin.read(&mut buf)?;
        if n == 0 {
            continue;
        }

        let mut offset = 0usize;
        while offset < n {
            let Ok((event, used)) = parser.parse(&buf[offset..n]) else {
                break;
            };
            offset += used;

            match event {
                Event::Key(key) => {
                    last_event = format!("Key: {:?} (mods: {:?})", key.code, key.modifiers);
                    if key.code == KeyCode::Char('q') || key.is_ctrl_c() {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse) => {
                    last_event = format!("Mouse: {:?} at ({}, {})", mouse.kind, mouse.x, mouse.y);
                }
                Event::Resize(resize) => {
                    last_event = format!("Resize: {}x{}", resize.width, resize.height);
                    status = String::from("Resize detected. Press q to exit.");
                }
                Event::Paste(paste) => {
                    last_event = format!("Paste: {} bytes", paste.content().len());
                }
                Event::FocusGained => {
                    last_event = String::from("Focus: gained");
                }
                Event::FocusLost => {
                    last_event = String::from("Focus: lost");
                }
            }
        }
    }
}
