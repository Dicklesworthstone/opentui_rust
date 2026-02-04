//! Example 02: Colors and Color Blending
//!
//! Demonstrates:
//! - `Rgba` creation methods (RGB, hex, HSV, constants)
//! - Alpha transparency (`with_alpha`, `multiply_alpha`)
//! - Porter-Duff blending with `set_blended`
//! - Gradients via `lerp`

use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{Cell, OptimizedBuffer, Renderer, Rgba, Style};
use opentui_rust as opentui;
use std::io::{self, Read};

fn center_x(width: u32, text: &str) -> u32 {
    let len = u32::try_from(text.len()).unwrap_or(u32::MAX);
    width.saturating_sub(len) / 2
}

fn draw_swatch(buffer: &mut OptimizedBuffer, x: u32, y: u32, color: Rgba) {
    let swatch_width = 4u32;
    for dx in 0..swatch_width {
        buffer.set(x + dx, y, Cell::new('#', Style::fg(color)));
    }
}

fn draw_color_creation(buffer: &mut OptimizedBuffer, mut y: u32, height: u32) -> u32 {
    if y < height {
        buffer.draw_text(2, y, "Color creation:", Style::fg(Rgba::WHITE).with_bold());
    }
    y = y.saturating_add(1);

    let color_hex = Rgba::from_hex("#ff6600").expect("valid color");
    let color_u8 = Rgba::from_rgb_u8(102, 51, 153);
    let color_hsv = Rgba::from_hsv(120.0, 1.0, 1.0);
    let color_rgb = Rgba::rgb(0.2, 0.6, 1.0);
    let color_new = Rgba::new(0.9, 0.2, 0.2, 0.8);
    let color_const = Rgba::BLUE;

    let entries = [
        ("from_hex #ff6600", color_hex),
        ("from_rgb_u8(102, 51, 153)", color_u8),
        ("from_hsv(120, 1, 1)", color_hsv),
        ("Rgba::rgb(0.2, 0.6, 1.0)", color_rgb),
        ("Rgba::new(0.9, 0.2, 0.2, 0.8)", color_new),
        ("Rgba::BLUE", color_const),
    ];

    for (label, color) in entries {
        if y >= height {
            break;
        }
        draw_swatch(buffer, 2, y, color);
        buffer.draw_text(7, y, label, Style::fg(Rgba::WHITE));
        y = y.saturating_add(1);
    }

    y
}

fn draw_alpha_blending(buffer: &mut OptimizedBuffer, mut y: u32, width: u32, height: u32) -> u32 {
    y = y.saturating_add(1);
    if y < height {
        buffer.draw_text(
            2,
            y,
            "Alpha blending (red over blue):",
            Style::fg(Rgba::WHITE).with_bold(),
        );
    }
    y = y.saturating_add(1);

    let alphas = [1.0f32, 0.75, 0.5, 0.25, 0.0];
    let swatch_gap = 2u32;
    let swatch_width = 4u32;
    let mut blend_x = 2u32;
    for alpha in alphas {
        if y >= height || blend_x >= width {
            break;
        }

        let overlay = Rgba::RED.with_alpha(alpha).multiply_alpha(0.9);
        for dx in 0..swatch_width {
            let cell_x = blend_x + dx;
            if cell_x >= width {
                break;
            }
            buffer.set(cell_x, y, Cell::new('#', Style::fg(Rgba::BLUE)));
            buffer.set_blended(cell_x, y, Cell::new('#', Style::fg(overlay)));
        }

        let label = format!("{:.0}%", alpha * 100.0);
        let label_x = blend_x + swatch_width + 1;
        if label_x < width {
            buffer.draw_text(label_x, y, &label, Style::fg(Rgba::WHITE));
        }

        blend_x = blend_x.saturating_add(swatch_width + swatch_gap + 4);
    }

    y
}

fn draw_gradient(buffer: &mut OptimizedBuffer, mut y: u32, width: u32, height: u32) -> u32 {
    y = y.saturating_add(2);
    if y < height {
        buffer.draw_text(
            2,
            y,
            "Gradient (lerp red -> blue):",
            Style::fg(Rgba::WHITE).with_bold(),
        );
    }
    y = y.saturating_add(1);

    if y < height {
        let max_len = width.saturating_sub(4).min(60);
        let gradient_len = usize::try_from(max_len).unwrap_or(0);
        let denom_u32 = u32::try_from(gradient_len.saturating_sub(1).max(1)).unwrap_or(1);
        let denom_u16 = u16::try_from(denom_u32).unwrap_or(u16::MAX);
        let denom = f32::from(denom_u16);

        for i in 0..gradient_len {
            // Safe: gradient_len is capped at 60, fits exactly in f32.
            let t = f32::from(u8::try_from(i).unwrap_or(0)) / denom;
            let color = Rgba::RED.lerp(Rgba::BLUE, t);
            let x = 2u32.saturating_add(u32::try_from(i).unwrap_or(u32::MAX));
            if x >= width {
                break;
            }
            buffer.set(x, y, Cell::new('#', Style::fg(color)));
        }
    }

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
        buffer.clear(Rgba::from_hex("#0f111a").expect("valid color"));

        let title = "OpenTUI Color Demonstration";
        let title_x = center_x(width, title);
        if y < height {
            buffer.draw_text(
                title_x,
                y,
                title,
                Style::fg(Rgba::from_hex("#7bdff2").expect("valid color")).with_bold(),
            );
        }
        y = y.saturating_add(2);

        y = draw_color_creation(buffer, y, height);
        y = draw_alpha_blending(buffer, y, width, height);
        let _ = draw_gradient(buffer, y, width, height);
    }

    renderer.present()?;
    let _ = io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
