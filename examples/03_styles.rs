//! Example 03: Text Styles and Attributes
//!
//! Demonstrates:
//! - `Style` builder pattern
//! - Shorthand style helpers (bold, italic, underline, etc.)
//! - Combining attributes and colors
//! - Background colors
//!
//! Note: Some terminals may not render all attributes (italic, blink, hidden).

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

fn draw_section_title(buffer: &mut OptimizedBuffer, width: u32, height: u32, y: u32, text: &str) {
    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        text,
        Style::fg(Rgba::WHITE).with_bold(),
    );
}

fn draw_individual_attributes(
    buffer: &mut OptimizedBuffer,
    width: u32,
    height: u32,
    mut y: u32,
) -> u32 {
    draw_section_title(buffer, width, height, y, "Individual attributes:");
    y = y.saturating_add(1);

    let col1 = 2u32;
    let col2 = 24u32;
    let col3 = 46u32;

    draw_text(
        buffer,
        width,
        height,
        col1,
        y,
        "Normal text",
        Style::fg(Rgba::WHITE),
    );
    draw_text(
        buffer,
        width,
        height,
        col2,
        y,
        "Bold text",
        Style::fg(Rgba::WHITE).with_bold(),
    );
    draw_text(
        buffer,
        width,
        height,
        col3,
        y,
        "Italic text",
        Style::fg(Rgba::WHITE).with_italic(),
    );
    y = y.saturating_add(1);

    draw_text(
        buffer,
        width,
        height,
        col1,
        y,
        "Underline",
        Style::fg(Rgba::WHITE).with_underline(),
    );
    draw_text(
        buffer,
        width,
        height,
        col2,
        y,
        "Dim text",
        Style::builder().fg(Rgba::WHITE).dim().build(),
    );
    draw_text(
        buffer,
        width,
        height,
        col3,
        y,
        "Inverse",
        Style::builder().fg(Rgba::WHITE).inverse().build(),
    );
    y = y.saturating_add(1);

    draw_text(
        buffer,
        width,
        height,
        col1,
        y,
        "Strikethrough",
        Style::builder().fg(Rgba::WHITE).strikethrough().build(),
    );
    draw_text(
        buffer,
        width,
        height,
        col2,
        y,
        "Blink",
        Style::builder().fg(Rgba::WHITE).blink().build(),
    );

    // Hidden text may render as blank; label it explicitly.
    draw_text(
        buffer,
        width,
        height,
        col3,
        y,
        "Hidden:",
        Style::fg(Rgba::WHITE),
    );
    draw_text(
        buffer,
        width,
        height,
        col3 + 8,
        y,
        "secret",
        Style::builder().fg(Rgba::WHITE).hidden().build(),
    );

    y
}

fn draw_combined_attributes(
    buffer: &mut OptimizedBuffer,
    width: u32,
    height: u32,
    mut y: u32,
) -> u32 {
    y = y.saturating_add(2);
    draw_section_title(buffer, width, height, y, "Combined attributes:");
    y = y.saturating_add(1);

    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "Bold + Italic",
        Style::fg(Rgba::WHITE).with_bold().with_italic(),
    );
    draw_text(
        buffer,
        width,
        height,
        26,
        y,
        "Bold + Underline",
        Style::fg(Rgba::WHITE).with_bold().with_underline(),
    );
    draw_text(
        buffer,
        width,
        height,
        52,
        y,
        "Bold + Italic + Underline",
        Style::fg(Rgba::WHITE)
            .with_bold()
            .with_italic()
            .with_underline(),
    );

    y
}

fn draw_builder_example(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    y = y.saturating_add(2);
    draw_section_title(buffer, width, height, y, "Style builder:");
    y = y.saturating_add(1);

    let builder_style = Style::builder()
        .fg(Rgba::from_hex("#f7d794").expect("valid color"))
        .bg(Rgba::from_hex("#303952").expect("valid color"))
        .bold()
        .italic()
        .build();

    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "Builder: bold + italic on dark background",
        builder_style,
    );

    y
}

fn draw_color_attributes(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    y = y.saturating_add(2);
    draw_section_title(buffer, width, height, y, "Colors + attributes:");
    y = y.saturating_add(1);

    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "Red bold",
        Style::fg(Rgba::RED).with_bold(),
    );
    draw_text(
        buffer,
        width,
        height,
        18,
        y,
        "Green italic",
        Style::fg(Rgba::GREEN).with_italic(),
    );
    draw_text(
        buffer,
        width,
        height,
        38,
        y,
        "Blue underline",
        Style::fg(Rgba::BLUE).with_underline(),
    );
    draw_text(
        buffer,
        width,
        height,
        60,
        y,
        "Yellow inverse",
        Style::builder()
            .fg(Rgba::from_hex("#fbc531").expect("valid color"))
            .inverse()
            .build(),
    );

    y
}

fn draw_backgrounds(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    y = y.saturating_add(2);
    draw_section_title(buffer, width, height, y, "Background colors:");
    y = y.saturating_add(1);

    let white_on_black = Style::builder().fg(Rgba::WHITE).bg(Rgba::BLACK).build();
    let black_on_white = Style::builder().fg(Rgba::BLACK).bg(Rgba::WHITE).build();
    let blue_on_yellow = Style::builder()
        .fg(Rgba::from_hex("#1e3799").expect("valid color"))
        .bg(Rgba::from_hex("#fbc531").expect("valid color"))
        .build();

    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "White on Black",
        white_on_black,
    );
    draw_text(
        buffer,
        width,
        height,
        26,
        y,
        "Black on White",
        black_on_white,
    );
    draw_text(
        buffer,
        width,
        height,
        48,
        y,
        "Blue on Yellow",
        blue_on_yellow,
    );

    y
}

fn main() -> io::Result<()> {
    let (term_w, term_h) = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(u32::from(term_w), u32::from(term_h))?;
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let mut y = 1u32;

    {
        let buffer = renderer.buffer();
        buffer.clear(Rgba::from_hex("#10131a").expect("valid color"));

        let title = "Text Styles Demo";
        let title_x = center_x(width, title);
        draw_text(
            buffer,
            width,
            height,
            title_x,
            y,
            title,
            Style::fg(Rgba::from_hex("#eccc68").expect("valid color")).with_bold(),
        );

        y = y.saturating_add(2);
        y = draw_individual_attributes(buffer, width, height, y);
        y = draw_combined_attributes(buffer, width, height, y);
        y = draw_builder_example(buffer, width, height, y);
        y = draw_color_attributes(buffer, width, height, y);
        let _ = draw_backgrounds(buffer, width, height, y);
    }

    renderer.present()?;
    let _ = io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
