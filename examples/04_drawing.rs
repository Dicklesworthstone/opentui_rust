//! Example 04: Drawing Primitives
//!
//! Demonstrates:
//! - Box styles (`BoxStyle::{single,double,rounded,heavy,ascii}`)
//! - Horizontal and vertical lines
//! - Filled rectangles
//! - Simple composite layouts built from primitives

use opentui::buffer::{BoxOptions, BoxStyle, TitleAlign};
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

fn draw_box_styles(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    draw_section_title(buffer, width, height, y, "Box styles:");
    y = y.saturating_add(1);

    let box_w = 12u32;
    let box_h = 5u32;
    let spacing = 2u32;
    let mut x = 2u32;

    let styles = [
        (
            "Single",
            BoxStyle::single(Style::fg(Rgba::from_hex("#74b9ff").expect("valid"))),
        ),
        (
            "Double",
            BoxStyle::double(Style::fg(Rgba::from_hex("#ffeaa7").expect("valid"))),
        ),
        (
            "Rounded",
            BoxStyle::rounded(Style::fg(Rgba::from_hex("#55efc4").expect("valid"))),
        ),
        (
            "Heavy",
            BoxStyle::heavy(Style::fg(Rgba::from_hex("#fab1a0").expect("valid"))),
        ),
        (
            "ASCII",
            BoxStyle::ascii(Style::fg(Rgba::from_hex("#dfe6e9").expect("valid"))),
        ),
    ];

    for (label, style) in styles {
        if x + box_w > width || y + box_h > height {
            break;
        }
        buffer.draw_box(x, y, box_w, box_h, style);
        let label_x = x + 1;
        let label_y = y + 2;
        draw_text(
            buffer,
            width,
            height,
            label_x,
            label_y,
            label,
            Style::fg(Rgba::WHITE),
        );
        x = x.saturating_add(box_w + spacing);
    }

    y.saturating_add(box_h + 1)
}

fn draw_lines(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    draw_section_title(buffer, width, height, y, "Lines:");
    y = y.saturating_add(1);

    let h_len = width.saturating_sub(8).min(22);
    if y < height {
        draw_hline(
            buffer,
            4,
            y,
            h_len,
            '─',
            Style::fg(Rgba::from_hex("#a29bfe").expect("valid")),
        );
        draw_text(
            buffer,
            width,
            height,
            4 + h_len + 2,
            y,
            "Horizontal",
            Style::fg(Rgba::WHITE),
        );
    }

    y = y.saturating_add(1);
    let v_len = height.saturating_sub(y.saturating_add(2)).min(4);
    if y < height && v_len > 0 {
        draw_vline(
            buffer,
            4,
            y,
            v_len,
            '│',
            Style::fg(Rgba::from_hex("#fd79a8").expect("valid")),
        );
        draw_text(
            buffer,
            width,
            height,
            6,
            y + v_len / 2,
            "Vertical",
            Style::fg(Rgba::WHITE),
        );
    }

    y.saturating_add(v_len + 1)
}

fn draw_filled_rect(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    draw_section_title(buffer, width, height, y, "Filled rectangle:");
    y = y.saturating_add(1);

    let rect_w = width.saturating_sub(10).min(26);
    let rect_h = height.saturating_sub(y.saturating_add(2)).min(5);
    if rect_w >= 4 && rect_h >= 3 {
        let x = 4u32;
        buffer.fill_rect(
            x,
            y,
            rect_w,
            rect_h,
            Rgba::from_hex("#2d3436cc").expect("valid"),
        );
        buffer.draw_box(
            x,
            y,
            rect_w,
            rect_h,
            BoxStyle::single(Style::fg(Rgba::from_hex("#dfe6e9").expect("valid"))),
        );
        draw_text(
            buffer,
            width,
            height,
            x + 2,
            y + rect_h / 2,
            "fill_rect + box",
            Style::fg(Rgba::WHITE),
        );
    }

    y.saturating_add(rect_h + 1)
}

fn draw_composite_panel(buffer: &mut OptimizedBuffer, width: u32, height: u32, mut y: u32) -> u32 {
    draw_section_title(buffer, width, height, y, "Composite layout:");
    y = y.saturating_add(1);

    let panel_w = width.saturating_sub(6).min(40);
    let panel_h = height.saturating_sub(y + 2).min(7);
    if panel_w >= 6 && panel_h >= 4 {
        let x = 3u32;
        let mut options = BoxOptions::new(BoxStyle::double(Style::fg(
            Rgba::from_hex("#81ecec").expect("valid"),
        )));
        options.fill = Some(Rgba::from_hex("#1e272e88").expect("valid"));
        options.title = Some("Panel".to_string());
        options.title_align = TitleAlign::Center;
        buffer.draw_box_with_options(x, y, panel_w, panel_h, options);

        let split_x = x + panel_w / 2;
        draw_vline(
            buffer,
            split_x,
            y + 1,
            panel_h.saturating_sub(2),
            '│',
            Style::fg(Rgba::from_hex("#81ecec").expect("valid")),
        );
        draw_hline(
            buffer,
            x + 1,
            y + panel_h / 2,
            panel_w.saturating_sub(2),
            '─',
            Style::fg(Rgba::from_hex("#81ecec").expect("valid")),
        );

        draw_text(
            buffer,
            width,
            height,
            x + 2,
            y + 2,
            "Left pane",
            Style::fg(Rgba::WHITE),
        );
        draw_text(
            buffer,
            width,
            height,
            split_x + 2,
            y + 2,
            "Right pane",
            Style::fg(Rgba::WHITE),
        );
    }

    y.saturating_add(panel_h + 1)
}

fn main() -> io::Result<()> {
    let (term_w, term_h) = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(u32::from(term_w), u32::from(term_h))?;
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let mut y = 1u32;

    {
        let buffer = renderer.buffer();
        buffer.clear(Rgba::from_hex("#101820").expect("valid"));

        let title = "Drawing Primitives";
        let title_x = center_x(width, title);
        draw_text(
            buffer,
            width,
            height,
            title_x,
            y,
            title,
            Style::fg(Rgba::from_hex("#f6e58d").expect("valid")).with_bold(),
        );

        y = y.saturating_add(2);
        y = draw_box_styles(buffer, width, height, y);
        y = draw_lines(buffer, width, height, y);
        y = draw_filled_rect(buffer, width, height, y);
        let _ = draw_composite_panel(buffer, width, height, y);
    }

    renderer.present()?;
    let _ = io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
