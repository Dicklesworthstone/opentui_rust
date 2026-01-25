//! Double-buffered terminal renderer with diff detection.
//!
//! This module provides [`Renderer`], the main entry point for rendering to
//! the terminal. It implements double-buffering with diff detection to minimize
//! the amount of ANSI output needed per frame.
//!
//! # Architecture
//!
//! The renderer maintains two buffers:
//! - **Back buffer**: Where your application draws (via [`Renderer::buffer`])
//! - **Front buffer**: The previous frame (used for diff detection)
//!
//! On [`present`](Renderer::present), the renderer computes which cells changed
//! and only outputs ANSI sequences for those cells. This dramatically reduces
//! output bandwidth for UIs that change incrementally.
//!
//! # Examples
//!
//! ```no_run
//! use opentui::{Renderer, Style, Rgba};
//!
//! fn main() -> std::io::Result<()> {
//!     // Create renderer (enters alt screen, hides cursor)
//!     let mut renderer = Renderer::new(80, 24)?;
//!
//!     // Main loop
//!     loop {
//!         // Clear and draw to back buffer
//!         renderer.clear();
//!         renderer.buffer().draw_text(10, 5, "Hello!", Style::fg(Rgba::GREEN));
//!
//!         // Present (diff-based, only changed cells written)
//!         renderer.present()?;
//!
//!         // Handle input, break on quit...
//!         break;
//!     }
//!
//!     Ok(())
//!     // Renderer::drop() restores terminal automatically
//! }
//! ```
//!
//! # Hit Testing
//!
//! The renderer includes a hit grid for mouse interaction. Register clickable
//! areas with [`register_hit_area`](Renderer::register_hit_area) and query
//! them with [`hit_test`](Renderer::hit_test).

mod diff;
mod hitgrid;

pub use diff::BufferDiff;
pub use hitgrid::HitGrid;

use crate::ansi::AnsiWriter;
use crate::buffer::{ClipRect, OptimizedBuffer, ScissorStack};
use crate::color::Rgba;
use crate::link::LinkPool;
use crate::terminal::{CursorStyle, Terminal};
use std::io::{self, Stdout, Write};
use std::time::{Duration, Instant};

/// Renderer configuration options.
///
/// These options control terminal setup behavior when creating a [`Renderer`].
#[derive(Clone, Copy, Debug)]
pub struct RendererOptions {
    /// Use the alternate screen buffer.
    pub use_alt_screen: bool,
    /// Hide the cursor on start.
    pub hide_cursor: bool,
    /// Enable mouse tracking.
    pub enable_mouse: bool,
    /// Query terminal capabilities on startup.
    pub query_capabilities: bool,
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            use_alt_screen: true,
            hide_cursor: true,
            enable_mouse: true,
            query_capabilities: true,
        }
    }
}

/// Rendering statistics.
#[derive(Clone, Debug, Default)]
pub struct RenderStats {
    pub frames: u64,
    pub last_frame_time: Duration,
    pub last_frame_cells: usize,
    pub fps: f32,
    pub buffer_bytes: usize,
    pub hitgrid_bytes: usize,
    pub total_bytes: usize,
}

/// CLI renderer with double buffering.
///
/// The renderer is the main entry point for terminal rendering. It manages:
/// - Double-buffered cell storage for flicker-free updates
/// - Diff-based output to minimize ANSI sequences
/// - Terminal state (cursor, alt screen, mouse tracking)
/// - Hit testing grid for mouse interaction
/// - Hyperlink pool for OSC 8 links
///
/// # Terminal Cleanup
///
/// The renderer implements [`Drop`] to restore terminal state automatically.
/// For explicit cleanup, call [`cleanup`](Self::cleanup).
///
/// # Thread Safety
///
/// `Renderer` is not `Send` because it holds a reference to stdout. Keep it
/// on the main thread and send drawing commands via channels if needed.
pub struct Renderer {
    width: u32,
    height: u32,

    front_buffer: OptimizedBuffer,
    back_buffer: OptimizedBuffer,

    terminal: Terminal<Stdout>,
    hit_grid: HitGrid,
    hit_scissor: ScissorStack,
    link_pool: LinkPool,
    scratch_buffer: Vec<u8>,

    background: Rgba,
    force_redraw: bool,
    stats: RenderStats,
    last_present_at: Instant,
    show_debug_overlay: bool,
}

impl Renderer {
    /// Create a new renderer with the given dimensions.
    pub fn new(width: u32, height: u32) -> io::Result<Self> {
        Self::new_with_options(width, height, RendererOptions::default())
    }

    /// Create a new renderer with custom options.
    pub fn new_with_options(width: u32, height: u32, options: RendererOptions) -> io::Result<Self> {
        let mut terminal = Terminal::new(io::stdout());
        if options.use_alt_screen {
            terminal.enter_alt_screen()?;
        }
        if options.hide_cursor {
            terminal.hide_cursor()?;
        }
        if options.enable_mouse {
            terminal.enable_mouse()?;
        }
        if options.query_capabilities {
            terminal.query_capabilities()?;
        }

        Ok(Self {
            width,
            height,
            front_buffer: OptimizedBuffer::new(width, height),
            back_buffer: OptimizedBuffer::new(width, height),
            terminal,
            hit_grid: HitGrid::new(width, height),
            hit_scissor: ScissorStack::new(),
            link_pool: LinkPool::new(),
            scratch_buffer: Vec::with_capacity(
                (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(20),
            ),
            background: Rgba::BLACK,
            force_redraw: true,
            stats: RenderStats::default(),
            last_present_at: Instant::now(),
            show_debug_overlay: false,
        })
    }

    /// Get buffer dimensions.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the back buffer for drawing.
    pub fn buffer(&mut self) -> &mut OptimizedBuffer {
        &mut self.back_buffer
    }

    /// Get the front buffer (current display state).
    #[must_use]
    pub fn front_buffer(&self) -> &OptimizedBuffer {
        &self.front_buffer
    }

    /// Get rendering stats.
    #[must_use]
    pub fn stats(&self) -> &RenderStats {
        &self.stats
    }

    /// Enable or disable the debug overlay.
    pub fn set_debug_overlay(&mut self, enabled: bool) {
        self.show_debug_overlay = enabled;
    }

    /// Access the link pool for hyperlink registration.
    pub fn link_pool(&mut self) -> &mut LinkPool {
        &mut self.link_pool
    }

    /// Set background color.
    pub fn set_background(&mut self, color: Rgba) {
        self.background = color;
    }

    /// Clear the back buffer.
    pub fn clear(&mut self) {
        self.back_buffer.clear(self.background);
        self.hit_grid.clear();
    }

    /// Present the back buffer to screen (swap buffers).
    pub fn present(&mut self) -> io::Result<()> {
        if self.show_debug_overlay {
            self.draw_debug_overlay();
        }

        let total_cells = (self.width * self.height) as usize;
        let diff = BufferDiff::compute(&self.front_buffer, &self.back_buffer);

        if self.force_redraw || diff.should_full_redraw(total_cells) {
            self.present_force()?;
            self.update_stats(total_cells);
            self.force_redraw = false;
        } else {
            self.present_diff(&diff)?;
            self.update_stats(diff.change_count);
        }

        // Swap buffers
        std::mem::swap(&mut self.front_buffer, &mut self.back_buffer);
        self.back_buffer.clear(self.background);
        self.hit_grid.clear();

        Ok(())
    }

    /// Force a full redraw.
    pub fn present_force(&mut self) -> io::Result<()> {
        if self.terminal.capabilities().sync_output {
            self.terminal.begin_sync()?;
        }

        self.scratch_buffer.clear();
        let mut writer = AnsiWriter::new(&mut self.scratch_buffer);

        for y in 0..self.height {
            writer.move_cursor(y, 0);
            for x in 0..self.width {
                if let Some(cell) = self.back_buffer.get(x, y) {
                    if !cell.is_continuation() {
                        let url = cell
                            .attributes
                            .link_id()
                            .and_then(|id| self.link_pool.get(id));
                        writer.write_cell_with_link(cell, url);
                    }
                }
            }
        }

        writer.reset();
        writer.flush()?;

        self.terminal.flush()?;
        // Write the accumulated content from scratch buffer to terminal
        io::stdout().write_all(&self.scratch_buffer)?;
        io::stdout().flush()?;

        if self.terminal.capabilities().sync_output {
            self.terminal.end_sync()?;
        }
        self.terminal.flush()
    }

    /// Present using diff detection.
    fn present_diff(&mut self, diff: &BufferDiff) -> io::Result<()> {
        if self.terminal.capabilities().sync_output {
            self.terminal.begin_sync()?;
        }

        self.scratch_buffer.clear();
        let mut writer = AnsiWriter::new(&mut self.scratch_buffer);

        for &(x, y) in &diff.changed_cells {
            let back_cell = self.back_buffer.get(x, y);
            if let Some(cell) = back_cell {
                if !cell.is_continuation() {
                    let url = cell
                        .attributes
                        .link_id()
                        .and_then(|id| self.link_pool.get(id));
                    writer.write_cell_at_with_link(y, x, cell, url);
                }
            }
        }

        writer.reset();
        writer.flush()?;

        if !self.scratch_buffer.is_empty() {
            io::stdout().write_all(&self.scratch_buffer)?;
            io::stdout().flush()?;
        }

        if self.terminal.capabilities().sync_output {
            self.terminal.end_sync()?;
        }
        self.terminal.flush()
    }

    /// Resize the renderer.
    pub fn resize(&mut self, width: u32, height: u32) -> io::Result<()> {
        self.width = width;
        self.height = height;
        self.front_buffer.resize(width, height);
        self.back_buffer.resize(width, height);
        self.hit_grid = HitGrid::new(width, height);
        self.hit_scissor.clear();
        self.force_redraw = true;
        self.terminal.clear()
    }

    /// Set cursor position.
    pub fn set_cursor(&mut self, x: u32, y: u32, visible: bool) -> io::Result<()> {
        if visible {
            self.terminal.show_cursor()?;
            self.terminal.move_cursor(x, y)?;
        } else {
            self.terminal.hide_cursor()?;
        }
        Ok(())
    }

    /// Set cursor style.
    pub fn set_cursor_style(&mut self, style: CursorStyle, blinking: bool) -> io::Result<()> {
        self.terminal.set_cursor_style(style, blinking)
    }

    /// Set window title.
    pub fn set_title(&mut self, title: &str) -> io::Result<()> {
        self.terminal.set_title(title)
    }

    /// Register a hit area for mouse testing.
    pub fn register_hit_area(&mut self, x: u32, y: u32, width: u32, height: u32, id: u32) {
        let rect = ClipRect::new(x as i32, y as i32, width, height);
        if let Some(intersect) = self.hit_scissor.current().intersect(&rect) {
            if !intersect.is_empty() {
                self.hit_grid.register(
                    intersect.x.max(0) as u32,
                    intersect.y.max(0) as u32,
                    intersect.width,
                    intersect.height,
                    id,
                );
            }
        }
    }

    /// Test which hit area contains a point.
    #[must_use]
    pub fn hit_test(&self, x: u32, y: u32) -> Option<u32> {
        self.hit_grid.test(x, y)
    }

    /// Push a hit-scissor rectangle (for hit testing).
    pub fn push_hit_scissor(&mut self, rect: ClipRect) {
        self.hit_scissor.push(rect);
    }

    /// Pop a hit-scissor rectangle.
    pub fn pop_hit_scissor(&mut self) {
        self.hit_scissor.pop();
    }

    /// Clear all hit-scissor rectangles.
    pub fn clear_hit_scissors(&mut self) {
        self.hit_scissor.clear();
    }

    /// Force next present to do a full redraw.
    pub fn invalidate(&mut self) {
        self.force_redraw = true;
    }

    /// Cleanup and restore terminal state.
    pub fn cleanup(&mut self) -> io::Result<()> {
        self.terminal.cleanup()
    }

    fn update_stats(&mut self, cells_updated: usize) {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_present_at);
        self.last_present_at = now;

        self.stats.frames = self.stats.frames.saturating_add(1);
        self.stats.last_frame_time = frame_time;
        self.stats.last_frame_cells = cells_updated;
        self.stats.fps = if frame_time.as_secs_f32() > 0.0 {
            1.0 / frame_time.as_secs_f32()
        } else {
            0.0
        };

        let buffer_bytes = self.front_buffer.byte_size() + self.back_buffer.byte_size();
        let hitgrid_bytes = self.hit_grid.byte_size();
        self.stats.buffer_bytes = buffer_bytes;
        self.stats.hitgrid_bytes = hitgrid_bytes;
        self.stats.total_bytes = buffer_bytes + hitgrid_bytes;
    }

    fn draw_debug_overlay(&mut self) {
        let stats = &self.stats;
        let text = format!(
            "fps:{:.1} frame:{:?} cells:{} mem:{}B",
            stats.fps, stats.last_frame_time, stats.last_frame_cells, stats.total_bytes
        );
        self.back_buffer
            .draw_text(0, 0, &text, crate::style::Style::dim());
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Most renderer tests require a real terminal,
    // so we just test basic buffer operations here.

    #[test]
    fn test_buffer_access() {
        // Can't test with real terminal in unit tests
        // Just verify the types compile correctly
        let front = OptimizedBuffer::new(80, 24);
        let back = OptimizedBuffer::new(80, 24);
        assert_eq!(front.size(), back.size());
    }
}
