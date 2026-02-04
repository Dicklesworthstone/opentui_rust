//! Example demonstrating the threaded renderer.
//!
//! The threaded renderer offloads terminal I/O to a dedicated thread,
//! allowing the main thread to continue processing while frames render.
//!
//! Run with: cargo run --example threaded

use opentui::renderer::ThreadedRenderer;
use opentui::{Rgba, Style};
use opentui_rust as opentui;
use std::io;
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
    // Create a threaded renderer (80x24 is a common terminal size)
    let mut renderer = ThreadedRenderer::new(80, 24)?;

    // Animation loop - 10 frames
    for frame in 0u32..10 {
        // Clear the back buffer
        renderer.clear();

        // Get the buffer for drawing
        let buffer = renderer.buffer();

        // Draw frame counter with bold style
        let title = format!("Frame {frame}");
        let title_style = Style::builder().fg(Rgba::WHITE).bold().build();
        buffer.draw_text(2, 1, &title, title_style);

        // Draw a simple animation - moving bar using draw_text
        let bar_x = frame * 5;
        let bar = "██████████"; // 10 block chars
        let bar_display: String = bar.chars().take((80 - bar_x as usize).min(10)).collect();
        buffer.draw_text(bar_x, 3, &bar_display, Style::fg(Rgba::GREEN));

        // Draw status
        buffer.draw_text(2, 5, "Press Ctrl+C to exit", Style::dim());

        // Present the frame (sends to render thread)
        renderer.present()?;

        // Simulate work on main thread while render thread writes to terminal
        thread::sleep(Duration::from_millis(100));
    }

    // Clean shutdown
    renderer.shutdown()?;

    println!("\nThreaded renderer example completed.");
    Ok(())
}
