//! Example 06: Opacity and Transparency
//!
//! Demonstrates:
//! - Global opacity stack
//! - Alpha blending with semi-transparent fills
//! - Layered rectangles using Porter-Duff compositing

use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{Cell, OptimizedBuffer, Renderer, Rgba, Style};
use opentui_rust as opentui;
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

fn draw_opacity_row(buffer: &mut OptimizedBuffer, width: u32, height: u32, y: u32) -> u32 {
    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "Opacity stack (global alpha):",
        Style::fg(Rgba::WHITE).with_bold(),
    );

    let base_y = y.saturating_add(2);
    let cell_w = width.saturating_sub(8).min(50);
    let cell_h = 5u32;
    let start_x = 4u32;

    if base_y + cell_h >= height {
        return y;
    }

    let base = Rgba::from_hex("#1e3799").expect("valid");
    buffer.fill_rect(start_x, base_y, cell_w, cell_h, base);

    let overlays = [1.0f32, 0.75, 0.5, 0.25];
    let count = u32::try_from(overlays.len()).unwrap_or(1);
    let swatch_w = (cell_w / count).max(4);

    for (idx, alpha) in overlays.iter().enumerate() {
        let idx_u32 = u32::try_from(idx).unwrap_or(u32::MAX);
        let x = start_x.saturating_add(idx_u32.saturating_mul(swatch_w));
        let overlay = Rgba::from_hex("#f8c291").expect("valid");
        buffer.push_opacity(*alpha);
        buffer.fill_rect(x, base_y, swatch_w, cell_h, overlay);
        buffer.pop_opacity();

        let label = format!("{:.0}%", alpha * 100.0);
        draw_text(
            buffer,
            width,
            height,
            x + 1,
            base_y + 2,
            &label,
            Style::fg(Rgba::WHITE),
        );
    }

    y.saturating_add(cell_h + 4)
}

fn draw_layered_cards(buffer: &mut OptimizedBuffer, width: u32, height: u32, y: u32) {
    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "Layered rectangles with alpha blending:",
        Style::fg(Rgba::WHITE).with_bold(),
    );

    let base_y = y.saturating_add(2);
    let card_w = width.saturating_sub(10).min(40);
    let card_h = 6u32;
    let x = 5u32;

    if base_y + card_h >= height {
        return;
    }

    buffer.fill_rect(
        x,
        base_y,
        card_w,
        card_h,
        Rgba::from_hex("#2f3640").expect("valid"),
    );

    let overlay_a = Rgba::from_hex("#e84118").expect("valid").with_alpha(0.7);
    let overlay_b = Rgba::from_hex("#00a8ff").expect("valid").with_alpha(0.6);

    buffer.set_blended(x + 3, base_y + 1, Cell::new('#', Style::fg(overlay_a)));
    buffer.fill_rect(x + 3, base_y + 1, card_w / 2, 3, overlay_a);
    buffer.fill_rect(x + card_w / 3, base_y + 2, card_w / 2, 3, overlay_b);

    draw_text(
        buffer,
        width,
        height,
        x + 2,
        base_y + card_h - 2,
        "Red/Blue overlays",
        Style::fg(Rgba::WHITE),
    );
}

fn main() -> io::Result<()> {
    let (term_w, term_h) = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(u32::from(term_w), u32::from(term_h))?;
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let mut y = 1u32;

    {
        let buffer = renderer.buffer();
        buffer.clear(Rgba::from_hex("#0f111a").expect("valid"));

        let title = "Opacity and Transparency";
        let title_x = center_x(width, title);
        draw_text(
            buffer,
            width,
            height,
            title_x,
            y,
            title,
            Style::fg(Rgba::from_hex("#f6b93b").expect("valid")).with_bold(),
        );

        y = y.saturating_add(2);
        y = draw_opacity_row(buffer, width, height, y);
        draw_layered_cards(buffer, width, height, y);
    }

    renderer.present()?;
    let _ = io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
