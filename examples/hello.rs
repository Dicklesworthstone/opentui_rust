//! Simple hello world example.

use opentui::buffer::BoxStyle;
use opentui::{OptimizedBuffer, Rgba, Style};
use opentui_rust as opentui;

fn main() {
    // Create a buffer
    let mut buffer = OptimizedBuffer::new(80, 24);

    // Clear with a dark background
    buffer.clear(Rgba::from_rgb_u8(20, 20, 30));

    // Draw some styled text
    let title_style = Style::builder()
        .fg(Rgba::from_hex("#FF6600").unwrap())
        .bold()
        .build();

    let text_style = Style::fg(Rgba::WHITE);

    buffer.draw_text(10, 5, "OpenTUI for Rust", title_style);
    buffer.draw_text(10, 7, "A high-performance terminal UI library", text_style);

    // Draw a box
    let box_style = BoxStyle::rounded(Style::fg(Rgba::from_hex("#4488FF").unwrap()));
    buffer.draw_box(5, 3, 50, 7, box_style);

    // Print buffer dimensions
    println!("Buffer created: {}x{}", buffer.width(), buffer.height());
    println!("Total cells: {}", buffer.cells().len());

    // In a real app, we'd render to the terminal here
    println!("\nTo see actual rendering, run with a terminal renderer.");
}
