//! Example 05: Scissor Clipping
//!
//! Demonstrates how scissor rectangles clip drawing operations.
//! - Push a scissor region
//! - Draw content that overflows
//! - Pop scissor to restore full drawing area

use opentui::buffer::ClipRect;
use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{Cell, OptimizedBuffer, Renderer, Rgba, Style};
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

fn draw_hline(buffer: &mut OptimizedBuffer, x: u32, y: u32, len: u32, ch: char, style: Style) {
    for col in x..x.saturating_add(len) {
        buffer.set_blended(col, y, Cell::new(ch, style));
    }
}

fn draw_vline(buffer: &mut OptimizedBuffer, x: u32, y: u32, len: u32, ch: char, style: Style) {
    for row in y..y.saturating_add(len) {
        buffer.set_blended(x, row, Cell::new(ch, style));
    }
}

fn draw_stripes(
    buffer: &mut OptimizedBuffer,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    even: Rgba,
    odd: Rgba,
) {
    for row in 0..h {
        let color = if row % 2 == 0 { even } else { odd };
        buffer.fill_rect(x, y + row, w, 1, color);
    }
}

fn draw_scissor_region(
    buffer: &mut OptimizedBuffer,
    width: u32,
    height: u32,
    scissor_x: u32,
    scissor_y: u32,
    scissor_w: u32,
    scissor_h: u32,
) {
    buffer.draw_box(
        scissor_x,
        scissor_y,
        scissor_w,
        scissor_h,
        opentui::buffer::BoxStyle::single(Style::fg(Rgba::from_hex("#576574").expect("valid"))),
    );

    let clip = ClipRect::new(
        i32::try_from(scissor_x).unwrap_or(0),
        i32::try_from(scissor_y).unwrap_or(0),
        scissor_w,
        scissor_h,
    );
    buffer.push_scissor(clip);

    let stripe_a = Rgba::from_hex("#2d3436").expect("valid").with_alpha(0.85);
    let stripe_b = Rgba::from_hex("#1e272e").expect("valid").with_alpha(0.85);
    draw_stripes(
        buffer,
        scissor_x + 1,
        scissor_y + 1,
        scissor_w - 2,
        scissor_h - 2,
        stripe_a,
        stripe_b,
    );

    draw_text(
        buffer,
        width,
        height,
        scissor_x.saturating_sub(2),
        scissor_y + 1,
        "This text is clipped on the left and right.",
        Style::fg(Rgba::from_hex("#feca57").expect("valid")),
    );

    let inner_x = scissor_x + 3;
    let inner_y = scissor_y + 3;
    let inner_w = scissor_w.saturating_sub(6);
    let inner_h = scissor_h.saturating_sub(6);
    if inner_w >= 4 && inner_h >= 3 {
        let inner_clip = ClipRect::new(
            i32::try_from(inner_x).unwrap_or(0),
            i32::try_from(inner_y).unwrap_or(0),
            inner_w,
            inner_h,
        );
        buffer.push_scissor(inner_clip);

        let cross_style = Style::fg(Rgba::from_hex("#54a0ff").expect("valid"));
        draw_hline(
            buffer,
            inner_x,
            inner_y + inner_h / 2,
            inner_w,
            '─',
            cross_style,
        );
        draw_vline(
            buffer,
            inner_x + inner_w / 2,
            inner_y,
            inner_h,
            '│',
            cross_style,
        );

        draw_text(
            buffer,
            width,
            height,
            inner_x + 1,
            inner_y + 1,
            "Nested clip",
            Style::fg(Rgba::WHITE).with_bold(),
        );

        buffer.pop_scissor();
    }

    buffer.pop_scissor();
}

fn draw_scissor_demo(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "Only the content inside the box is visible after pushing a scissor.",
        Style::fg(Rgba::WHITE),
    );
    y = y.saturating_add(2);

    let scissor_x = 4u32;
    let scissor_y = y;
    let scissor_w = width.saturating_sub(8).clamp(12, 52);
    let scissor_h = height
        .saturating_sub(scissor_y.saturating_add(6))
        .clamp(6, 10);

    if scissor_x + scissor_w < width && scissor_y + scissor_h < height {
        draw_scissor_region(
            buffer, width, height, scissor_x, scissor_y, scissor_w, scissor_h,
        );
    }

    y = scissor_y.saturating_add(scissor_h + 2);
    draw_text(
        buffer,
        width,
        height,
        2,
        y,
        "After popping the scissor, drawing returns to full screen.",
        Style::fg(Rgba::WHITE),
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
        buffer.clear(Rgba::from_hex("#0f111a").expect("valid"));

        let title = "Scissor Clipping";
        let title_x = center_x(width, title);
        draw_text(
            buffer,
            width,
            height,
            title_x,
            y,
            title,
            Style::fg(Rgba::from_hex("#c7ecee").expect("valid")).with_bold(),
        );
        y = y.saturating_add(2);

        let _ = draw_scissor_demo(buffer, width, height, y);
    }

    renderer.present()?;
    let _ = io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
