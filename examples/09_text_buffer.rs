//! Example 09: `TextBuffer` and Styled Segments
//!
//! Demonstrates:
//! - Building styled text with `TextBuffer` highlights
//! - Practical styling patterns (logs, code snippet)
//! - Rendering via `TextBufferView`
//! - Wide character width handling

use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{Renderer, Rgba, Style, TextBuffer, TextBufferView};
use opentui_rust as opentui;
use std::io::{self, Read};

fn text_len_u32(text: &str) -> u32 {
    u32::try_from(text.len()).unwrap_or(u32::MAX)
}

fn center_x(width: u32, text: &str) -> u32 {
    width.saturating_sub(text_len_u32(text)) / 2
}

fn add_highlight_substring(
    buffer: &mut TextBuffer,
    line_idx: usize,
    line_text: &str,
    needle: &str,
    style: Style,
) {
    let Some(byte_start) = line_text.find(needle) else {
        return;
    };
    let char_start = line_text[..byte_start].chars().count();
    let char_end = char_start + needle.chars().count();
    buffer.add_highlight_line(line_idx, char_start, char_end, style, 1, None);
}

#[allow(clippy::too_many_lines)] // Example intentionally shows a long, annotated text buffer.
fn build_text_buffer() -> TextBuffer {
    let lines = [
        "TextBuffer Demo",
        "",
        "Simple styled text:",
        "The quick brown fox jumps over the lazy dog.",
        "",
        "Log message styling:",
        "[INFO]  Application started successfully",
        "[WARN]  Configuration file missing, using defaults",
        "[ERROR] Failed to connect to database",
        "",
        "Code snippet:",
        "fn main() {",
        "    let x = 42;",
        "    println!(\"Hello, {x}!\");",
        "}",
        "",
        "Wide characters: 漢字 😀 emoji width demo",
    ];

    let text = lines.join("\n");
    let mut buffer = TextBuffer::new();
    buffer.set_text(&text);

    // Title styling
    add_highlight_substring(
        &mut buffer,
        0,
        lines[0],
        "TextBuffer Demo",
        Style::fg(Rgba::from_hex("#f6b93b").expect("valid")).with_bold(),
    );

    // Simple styled text highlights
    add_highlight_substring(
        &mut buffer,
        3,
        lines[3],
        "quick",
        Style::fg(Rgba::from_hex("#2ecc71").expect("valid")),
    );
    add_highlight_substring(
        &mut buffer,
        3,
        lines[3],
        "brown",
        Style::fg(Rgba::from_hex("#e67e22").expect("valid")),
    );
    add_highlight_substring(
        &mut buffer,
        3,
        lines[3],
        "lazy",
        Style::fg(Rgba::from_hex("#3498db").expect("valid")),
    );

    // Log message tags
    add_highlight_substring(
        &mut buffer,
        6,
        lines[6],
        "[INFO]",
        Style::fg(Rgba::from_hex("#00cec9").expect("valid")).with_bold(),
    );
    add_highlight_substring(
        &mut buffer,
        7,
        lines[7],
        "[WARN]",
        Style::fg(Rgba::from_hex("#fdcb6e").expect("valid")).with_bold(),
    );
    add_highlight_substring(
        &mut buffer,
        8,
        lines[8],
        "[ERROR]",
        Style::fg(Rgba::from_hex("#e74c3c").expect("valid")).with_bold(),
    );

    // Code snippet highlights
    add_highlight_substring(
        &mut buffer,
        11,
        lines[11],
        "fn",
        Style::fg(Rgba::from_hex("#74b9ff").expect("valid")).with_bold(),
    );
    add_highlight_substring(
        &mut buffer,
        12,
        lines[12],
        "let",
        Style::fg(Rgba::from_hex("#74b9ff").expect("valid")).with_bold(),
    );
    add_highlight_substring(
        &mut buffer,
        12,
        lines[12],
        "42",
        Style::fg(Rgba::from_hex("#a29bfe").expect("valid")).with_bold(),
    );
    add_highlight_substring(
        &mut buffer,
        13,
        lines[13],
        "println!",
        Style::fg(Rgba::from_hex("#74b9ff").expect("valid")).with_bold(),
    );
    add_highlight_substring(
        &mut buffer,
        13,
        lines[13],
        "\"Hello, {x}!\"",
        Style::fg(Rgba::from_hex("#55efc4").expect("valid")),
    );

    // Wide characters
    add_highlight_substring(
        &mut buffer,
        16,
        lines[16],
        "漢字",
        Style::fg(Rgba::from_hex("#ff6b6b").expect("valid")).with_bold(),
    );
    add_highlight_substring(
        &mut buffer,
        16,
        lines[16],
        "😀",
        Style::fg(Rgba::from_hex("#feca57").expect("valid")).with_bold(),
    );

    buffer
}

fn main() -> io::Result<()> {
    let (term_w, term_h) = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(u32::from(term_w), u32::from(term_h))?;
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let text_buffer = build_text_buffer();

    {
        let buffer = renderer.buffer();
        buffer.clear(Rgba::from_hex("#0f111a").expect("valid"));

        let title = "TextBuffer Demo";
        let title_x = center_x(width, title);
        buffer.draw_text(
            title_x,
            0,
            title,
            Style::fg(Rgba::from_hex("#f6b93b").expect("valid")).with_bold(),
        );

        let view =
            TextBufferView::new(&text_buffer).viewport(0, 1, width, height.saturating_sub(1));
        view.render_to(buffer, 0, 1);
    }

    renderer.present()?;
    let _ = io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
