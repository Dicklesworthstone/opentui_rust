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
mod threaded;

pub use diff::BufferDiff;
pub use hitgrid::HitGrid;
pub use threaded::{ThreadedRenderStats, ThreadedRenderer};

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
    grapheme_pool: crate::grapheme_pool::GraphemePool,
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
            grapheme_pool: crate::grapheme_pool::GraphemePool::new(),
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

    /// Get the back buffer with the grapheme pool for pool-aware drawing.
    pub fn buffer_with_pool(
        &mut self,
    ) -> (
        &mut OptimizedBuffer,
        &mut crate::grapheme_pool::GraphemePool,
    ) {
        (&mut self.back_buffer, &mut self.grapheme_pool)
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

    /// Get a mutable reference to the grapheme pool.
    ///
    /// The grapheme pool stores multi-codepoint grapheme clusters (emoji, ZWJ sequences)
    /// and allows them to be referenced by [`GraphemeId`](crate::cell::GraphemeId) in cells.
    pub fn grapheme_pool(&mut self) -> &mut crate::grapheme_pool::GraphemePool {
        &mut self.grapheme_pool
    }

    /// Get an immutable reference to the grapheme pool.
    #[must_use]
    pub fn grapheme_pool_ref(&self) -> &crate::grapheme_pool::GraphemePool {
        &self.grapheme_pool
    }

    /// Get detected terminal capabilities.
    ///
    /// Capabilities include color support level, hyperlink support,
    /// synchronized output, mouse tracking, and other terminal features.
    /// Applications can use this to adapt their rendering or show
    /// capability status in an inspector panel.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use opentui::Renderer;
    ///
    /// let renderer = Renderer::new(80, 24)?;
    /// let caps = renderer.capabilities();
    ///
    /// if caps.hyperlinks {
    ///     // Register clickable links
    /// }
    /// if caps.sync_output {
    ///     // Synchronized output available, no flicker
    /// }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[must_use]
    pub fn capabilities(&self) -> &crate::terminal::Capabilities {
        self.terminal.capabilities()
    }

    /// Get mutable access to terminal capabilities.
    ///
    /// This allows manually overriding detected capabilities, which can be
    /// useful for testing different terminal configurations or forcing
    /// specific behavior.
    ///
    /// **Note:** Generally prefer the immutable [`capabilities`](Self::capabilities)
    /// accessor unless you have a specific need to modify capability flags.
    pub fn capabilities_mut(&mut self) -> &mut crate::terminal::Capabilities {
        self.terminal.capabilities_mut()
    }

    /// Set background color.
    pub fn set_background(&mut self, color: Rgba) {
        self.background = color;
    }

    /// Clear the back buffer.
    pub fn clear(&mut self) {
        self.back_buffer
            .clear_with_pool(&mut self.grapheme_pool, self.background);
        self.hit_grid.clear();
    }

    /// Present the back buffer to screen (swap buffers).
    pub fn present(&mut self) -> io::Result<()> {
        if self.show_debug_overlay {
            self.draw_debug_overlay();
        }

        let total_cells = (self.width as usize).saturating_mul(self.height as usize);
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
        self.back_buffer
            .clear_with_pool(&mut self.grapheme_pool, self.background);
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
                        writer.write_cell_with_link_and_pool(cell, url, &self.grapheme_pool);
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

        for region in &diff.dirty_regions {
            writer.move_cursor(region.y, region.x);
            for i in 0..region.width {
                let x = region.x + i;
                let y = region.y;
                let back_cell = self.back_buffer.get(x, y);
                if let Some(cell) = back_cell {
                    // Skip continuation cells - they don't produce output
                    if cell.is_continuation() {
                        continue;
                    }
                    let url = cell
                        .attributes
                        .link_id()
                        .and_then(|id| self.link_pool.get(id));
                    writer.write_cell_with_pool_and_link(cell, &self.grapheme_pool, url);
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
        self.front_buffer
            .resize_with_pool(&mut self.grapheme_pool, width, height);
        self.back_buffer
            .resize_with_pool(&mut self.grapheme_pool, width, height);
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
    use crate::cell::Cell;

    // ============================================
    // RendererOptions Tests
    // ============================================

    #[test]
    fn test_renderer_options_default() {
        let opts = RendererOptions::default();
        assert!(opts.use_alt_screen);
        assert!(opts.hide_cursor);
        assert!(opts.enable_mouse);
        assert!(opts.query_capabilities);
    }

    #[test]
    fn test_renderer_options_custom() {
        let opts = RendererOptions {
            use_alt_screen: false,
            hide_cursor: false,
            enable_mouse: false,
            query_capabilities: false,
        };
        assert!(!opts.use_alt_screen);
        assert!(!opts.hide_cursor);
        assert!(!opts.enable_mouse);
        assert!(!opts.query_capabilities);
    }

    #[test]
    fn test_renderer_options_copy() {
        let opts = RendererOptions::default();
        let copy = opts;
        assert_eq!(opts.use_alt_screen, copy.use_alt_screen);
    }

    // ============================================
    // RenderStats Tests
    // ============================================

    #[test]
    fn test_render_stats_default() {
        let stats = RenderStats::default();
        assert_eq!(stats.frames, 0);
        assert_eq!(stats.last_frame_cells, 0);
        assert_eq!(stats.fps, 0.0);
        assert_eq!(stats.buffer_bytes, 0);
        assert_eq!(stats.hitgrid_bytes, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[test]
    fn test_render_stats_clone() {
        let stats = RenderStats {
            frames: 100,
            last_frame_time: Duration::from_millis(16),
            last_frame_cells: 1920,
            fps: 60.0,
            buffer_bytes: 10000,
            hitgrid_bytes: 5000,
            total_bytes: 15000,
        };
        let cloned = stats.clone();
        assert_eq!(cloned.frames, 100);
        assert_eq!(cloned.fps, 60.0);
    }

    // ============================================
    // Buffer Composition Tests (without terminal)
    // ============================================

    #[test]
    fn test_buffer_access() {
        // Basic test that buffers can be created and compared
        let front = OptimizedBuffer::new(80, 24);
        let back = OptimizedBuffer::new(80, 24);
        assert_eq!(front.size(), back.size());
    }

    #[test]
    fn test_buffer_composition_double_buffer() {
        // Test that two buffers can be used for double buffering
        let mut front = OptimizedBuffer::new(80, 24);
        let mut back = OptimizedBuffer::new(80, 24);

        // Draw to back buffer
        back.set(10, 5, Cell::new('X', crate::style::Style::NONE));

        // Swap (simulate present)
        std::mem::swap(&mut front, &mut back);

        // Now front has the cell
        assert!(front.get(10, 5).is_some());
        let cell = front.get(10, 5).unwrap();
        assert!(matches!(cell.content, crate::cell::CellContent::Char('X')));
    }

    #[test]
    fn test_buffer_composition_resize() {
        let mut buf = OptimizedBuffer::new(80, 24);
        assert_eq!(buf.size(), (80, 24));

        let mut pool = crate::grapheme_pool::GraphemePool::new();
        buf.resize_with_pool(&mut pool, 100, 50);
        assert_eq!(buf.size(), (100, 50));
    }

    #[test]
    fn test_buffer_composition_clear() {
        let mut buf = OptimizedBuffer::new(80, 24);
        buf.set(0, 0, Cell::clear(Rgba::RED));

        let mut pool = crate::grapheme_pool::GraphemePool::new();
        buf.clear_with_pool(&mut pool, Rgba::BLACK);

        let cell = buf.get(0, 0).unwrap();
        assert_eq!(cell.bg, Rgba::BLACK);
    }

    // ============================================
    // BufferDiff Integration Tests
    // ============================================

    #[test]
    fn test_buffer_diff_integration() {
        let front = OptimizedBuffer::new(80, 24);
        let mut back = OptimizedBuffer::new(80, 24);

        back.set(40, 12, Cell::new('A', crate::style::Style::fg(Rgba::RED)));

        let diff = BufferDiff::compute(&front, &back);
        assert!(!diff.is_empty());
        assert!(diff.changed_cells.contains(&(40, 12)));
    }

    #[test]
    fn test_buffer_diff_no_changes() {
        let front = OptimizedBuffer::new(80, 24);
        let back = OptimizedBuffer::new(80, 24);

        let diff = BufferDiff::compute(&front, &back);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_buffer_diff_full_redraw_threshold() {
        let front = OptimizedBuffer::new(10, 10);
        let mut back = OptimizedBuffer::new(10, 10);

        // Change more than 50% of cells
        for y in 0..10 {
            for x in 0..6 {
                back.set(x, y, Cell::clear(Rgba::RED));
            }
        }

        let diff = BufferDiff::compute(&front, &back);
        let total_cells = 100;
        assert!(diff.should_full_redraw(total_cells));
    }

    // ============================================
    // HitGrid Integration Tests
    // ============================================

    #[test]
    fn test_hit_grid_integration() {
        let mut grid = HitGrid::new(80, 24);

        // Register a button region
        grid.register(10, 5, 20, 3, 1);

        // Test hit detection
        assert_eq!(grid.hit_test(15, 6), Some(1));
        assert_eq!(grid.hit_test(5, 6), None);
    }

    #[test]
    fn test_hit_grid_clear_integration() {
        let mut grid = HitGrid::new(80, 24);
        grid.register(0, 0, 80, 24, 1);
        assert_eq!(grid.hit_test(40, 12), Some(1));

        grid.clear();
        assert_eq!(grid.hit_test(40, 12), None);
    }

    // ============================================
    // ScissorStack Tests
    // ============================================

    #[test]
    fn test_scissor_stack_hit_clipping() {
        let mut scissor = ScissorStack::new();

        // Initial scissor covers everything
        let full = scissor.current();
        assert!(full.contains(0, 0));

        // Push a restrictive scissor
        scissor.push(ClipRect::new(10, 10, 20, 20));
        let clipped = scissor.current();
        assert!(!clipped.contains(5, 5));
        assert!(clipped.contains(15, 15));

        // Pop returns to full
        scissor.pop();
        let restored = scissor.current();
        assert!(restored.contains(5, 5));
    }

    // ============================================
    // LinkPool Integration Tests
    // ============================================

    #[test]
    fn test_link_pool_allocation() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://example.com");
        let id2 = pool.alloc("https://other.com");

        assert!(id1 != id2);
        assert_eq!(pool.get(id1), Some("https://example.com"));
        assert_eq!(pool.get(id2), Some("https://other.com"));
    }

    #[test]
    fn test_link_pool_refcounting() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");

        pool.incref(id);
        pool.decref(id);

        // Should still exist (one ref remaining)
        assert!(pool.get(id).is_some());

        pool.decref(id);
        // Now freed
        assert!(pool.get(id).is_none());
    }

    // ============================================
    // GraphemePool Integration Tests
    // ============================================

    #[test]
    fn test_grapheme_pool_allocation() {
        let mut pool = crate::grapheme_pool::GraphemePool::new();
        let id = pool.alloc("👨‍👩‍👧");

        assert!(pool.get(id).is_some());
        assert_eq!(pool.get(id), Some("👨‍👩‍👧"));
    }

    // ============================================
    // Edge Case Tests
    // ============================================

    #[test]
    fn test_zero_size_buffer() {
        // Zero size buffer should not panic
        let buf = OptimizedBuffer::new(0, 0);
        assert_eq!(buf.size(), (0, 0));
    }

    #[test]
    fn test_single_cell_buffer() {
        let mut buf = OptimizedBuffer::new(1, 1);
        buf.set(0, 0, Cell::new('X', crate::style::Style::NONE));
        assert!(buf.get(0, 0).is_some());
    }

    #[test]
    fn test_large_buffer() {
        // Large buffer allocation should work
        let buf = OptimizedBuffer::new(500, 200);
        assert_eq!(buf.size(), (500, 200));
    }

    #[test]
    fn test_buffer_out_of_bounds() {
        let buf = OptimizedBuffer::new(80, 24);
        assert!(buf.get(80, 24).is_none());
        assert!(buf.get(100, 100).is_none());
    }

    // ============================================
    // DirtyRegion Tests
    // ============================================

    #[test]
    fn test_dirty_region_creation() {
        let region = diff::DirtyRegion::new(10, 20, 30, 40);
        assert_eq!(region.x, 10);
        assert_eq!(region.y, 20);
        assert_eq!(region.width, 30);
        assert_eq!(region.height, 40);
    }

    #[test]
    fn test_dirty_region_cell() {
        let region = diff::DirtyRegion::cell(5, 10);
        assert_eq!(region.x, 5);
        assert_eq!(region.y, 10);
        assert_eq!(region.width, 1);
        assert_eq!(region.height, 1);
    }

    // ============================================
    // Buffer State Preservation Tests
    // ============================================

    #[test]
    fn test_buffer_preserves_content_on_same_set() {
        let mut buf = OptimizedBuffer::new(10, 10);
        let cell = Cell::new('A', crate::style::Style::fg(Rgba::RED));
        buf.set(5, 5, cell.clone());
        buf.set(5, 5, cell);

        let stored = buf.get(5, 5).unwrap();
        assert!(matches!(
            stored.content,
            crate::cell::CellContent::Char('A')
        ));
    }

    #[test]
    fn test_buffer_multiple_sets_same_cell() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.set(5, 5, Cell::new('A', crate::style::Style::NONE));
        buf.set(5, 5, Cell::new('B', crate::style::Style::NONE));
        buf.set(5, 5, Cell::new('C', crate::style::Style::NONE));

        let stored = buf.get(5, 5).unwrap();
        assert!(matches!(
            stored.content,
            crate::cell::CellContent::Char('C')
        ));
    }

    // ============================================
    // Stats Calculation Tests
    // ============================================

    #[test]
    fn test_stats_byte_size_calculation() {
        let buf = OptimizedBuffer::new(80, 24);
        let byte_size = buf.byte_size();
        // Should be at least cells * size_of(Cell)
        assert!(byte_size > 0);
    }

    #[test]
    fn test_hit_grid_byte_size() {
        let grid = HitGrid::new(80, 24);
        let byte_size = grid.byte_size();
        // Should be width * height * size_of(Option<u32>)
        let expected = 80 * 24 * std::mem::size_of::<Option<u32>>();
        assert_eq!(byte_size, expected);
    }

    impl HitGrid {
        // Helper for testing
        fn hit_test(&self, x: u32, y: u32) -> Option<u32> {
            self.test(x, y)
        }
    }
}
