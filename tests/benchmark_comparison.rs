//! Quick performance comparison tests with budgets.
//!
//! These tests run fast (no criterion overhead) but enforce performance budgets
//! to catch regressions. Budgets are generous to account for CI variance.

// Test code - allow various pedantic lints
#![allow(clippy::doc_markdown)] // Allow unbackticked names in docs
#![allow(clippy::uninlined_format_args)] // Allow {:?} style for clarity

use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use std::time::{Duration, Instant};

/// Performance budget configuration.
/// These budgets are intentionally generous to avoid CI flakiness.
/// The goal is to catch major regressions, not enforce tight bounds.
mod budgets {
    use super::Duration;

    /// Budget for 100 buffer clears (200x50 = 10,000 cells).
    /// Expected: ~1-5ms on modern hardware.
    pub const CLEAR_100X: Duration = Duration::from_millis(100);

    /// Budget for 100 short text draws.
    /// Expected: ~1-5ms on modern hardware.
    pub const DRAW_TEXT_100X: Duration = Duration::from_millis(100);

    /// Budget for 10,000 cell set operations.
    /// Expected: ~1-5ms on modern hardware.
    pub const SET_CELL_10K: Duration = Duration::from_millis(100);

    /// Budget for 1,000 buffer-to-buffer copies.
    /// Expected: ~10-50ms on modern hardware.
    pub const BUFFER_COPY_1K: Duration = Duration::from_millis(500);

    /// Budget for 100 `fill_rect` operations (50x20 region).
    /// Expected: ~5-20ms on modern hardware.
    pub const FILL_RECT_100X: Duration = Duration::from_millis(200);
}

fn time<F: FnMut()>(mut f: F, iterations: u32) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed()
}

#[test]
fn benchmark_quick_comparison() {
    let mut buffer = OptimizedBuffer::new(200, 50);
    let style = Style::fg(Rgba::WHITE);
    let cell = Cell::new('X', Style::fg(Rgba::RED));

    // Clear benchmark (100 iterations)
    let clear_time = time(|| buffer.clear(Rgba::BLACK), 100);
    println!(
        "clear_100x: {:?} (budget: {:?})",
        clear_time,
        budgets::CLEAR_100X
    );
    assert!(
        clear_time < budgets::CLEAR_100X,
        "PERFORMANCE REGRESSION: clear took {:?}, budget is {:?}",
        clear_time,
        budgets::CLEAR_100X
    );

    // Draw text benchmark (100 iterations)
    let draw_time = time(|| buffer.draw_text(0, 0, "Hello, OpenTUI!", style), 100);
    println!(
        "draw_text_100x: {:?} (budget: {:?})",
        draw_time,
        budgets::DRAW_TEXT_100X
    );
    assert!(
        draw_time < budgets::DRAW_TEXT_100X,
        "PERFORMANCE REGRESSION: draw_text took {:?}, budget is {:?}",
        draw_time,
        budgets::DRAW_TEXT_100X
    );

    // Cell set benchmark (10,000 iterations)
    let set_time = time(|| buffer.set(10, 10, cell.clone()), 10_000);
    println!(
        "set_cell_10k: {:?} (budget: {:?})",
        set_time,
        budgets::SET_CELL_10K
    );
    assert!(
        set_time < budgets::SET_CELL_10K,
        "PERFORMANCE REGRESSION: set_cell took {:?}, budget is {:?}",
        set_time,
        budgets::SET_CELL_10K
    );
}

#[test]
fn benchmark_buffer_copy() {
    let src = OptimizedBuffer::new(100, 40);
    let mut dst = OptimizedBuffer::new(200, 50);

    // Buffer copy benchmark (1,000 iterations)
    let copy_time = time(|| dst.draw_buffer(0, 0, &src), 1_000);
    println!(
        "buffer_copy_1k: {:?} (budget: {:?})",
        copy_time,
        budgets::BUFFER_COPY_1K
    );
    assert!(
        copy_time < budgets::BUFFER_COPY_1K,
        "PERFORMANCE REGRESSION: buffer_copy took {:?}, budget is {:?}",
        copy_time,
        budgets::BUFFER_COPY_1K
    );
}

#[test]
fn benchmark_fill_rect() {
    let mut buffer = OptimizedBuffer::new(200, 50);

    // Fill rect benchmark (100 iterations of 50x20 region)
    let fill_time = time(|| buffer.fill_rect(10, 10, 50, 20, Rgba::BLUE), 100);
    println!(
        "fill_rect_100x: {:?} (budget: {:?})",
        fill_time,
        budgets::FILL_RECT_100X
    );
    assert!(
        fill_time < budgets::FILL_RECT_100X,
        "PERFORMANCE REGRESSION: fill_rect took {:?}, budget is {:?}",
        fill_time,
        budgets::FILL_RECT_100X
    );
}

#[test]
fn benchmark_blended_operations() {
    let mut buffer = OptimizedBuffer::new(200, 50);
    let semi_transparent = Rgba::new(1.0, 0.0, 0.0, 0.5);
    let cell = Cell::new('X', Style::fg(semi_transparent));

    // First fill with background
    buffer.clear(Rgba::WHITE);

    // Blended cell set benchmark (1,000 iterations)
    let blend_time = time(|| buffer.set_blended(50, 25, cell.clone()), 1_000);
    println!("blended_set_1k: {blend_time:?}");

    // No strict budget for blended ops yet, but verify it completes
    assert!(blend_time < Duration::from_millis(100));
}

#[test]
fn benchmark_scissor_operations() {
    use opentui::buffer::ClipRect;

    let mut buffer = OptimizedBuffer::new(200, 50);
    let cell = Cell::new('X', Style::fg(Rgba::RED));

    // Push a scissor rect
    buffer.push_scissor(ClipRect::new(10, 10, 100, 30));

    // Cell operations with scissor (1,000 iterations)
    let scissor_time = time(|| buffer.set(50, 25, cell.clone()), 1_000);
    println!("scissor_set_1k: {scissor_time:?}");

    // Scissor overhead should be minimal
    assert!(scissor_time < Duration::from_millis(50));

    buffer.pop_scissor();
}
