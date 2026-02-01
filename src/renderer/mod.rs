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
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
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
    /// Hit areas for the last presented frame (used by `hit_test`).
    front_hit_grid: HitGrid,
    /// Hit areas being built for the next frame (populated by `register_hit_area`).
    back_hit_grid: HitGrid,
    layer_hit_grids: BTreeMap<u16, HitGrid>,
    hit_scissor: ScissorStack,
    link_pool: LinkPool,
    grapheme_pool: crate::grapheme_pool::GraphemePool,
    scratch_buffer: Vec<u8>,
    /// Reusable diff to avoid per-frame allocation.
    cached_diff: BufferDiff,

    layers: BTreeMap<u16, OptimizedBuffer>,
    active_hit_layer: u16,
    layers_dirty: bool,

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

        let total_cells = (width as usize).saturating_mul(height as usize);
        Ok(Self {
            width,
            height,
            front_buffer: OptimizedBuffer::new(width, height),
            back_buffer: OptimizedBuffer::new(width, height),
            terminal,
            front_hit_grid: HitGrid::new(width, height),
            back_hit_grid: HitGrid::new(width, height),
            layer_hit_grids: BTreeMap::new(),
            hit_scissor: ScissorStack::new(),
            link_pool: LinkPool::new(),
            grapheme_pool: crate::grapheme_pool::GraphemePool::new(),
            scratch_buffer: Vec::with_capacity(total_cells.saturating_mul(20)),
            cached_diff: BufferDiff::with_capacity(total_cells / 8),
            layers: BTreeMap::new(),
            active_hit_layer: 0,
            layers_dirty: false,
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

    /// Render into an offscreen layer buffer.
    ///
    /// Layer `0` is the base layer (the regular back buffer). Higher layer IDs are
    /// composited on top, in ascending order.
    ///
    /// The active layer is also used for hit registration via [`Self::register_hit_area`]:
    /// after calling `render_to_layer(layer_id, ...)`, subsequent hit registrations will
    /// target that layer until another `render_to_layer` call or a frame reset.
    pub fn render_to_layer<F>(&mut self, layer_id: u16, render_fn: F)
    where
        F: FnOnce(&mut OptimizedBuffer),
    {
        self.active_hit_layer = layer_id;

        if layer_id == 0 {
            render_fn(&mut self.back_buffer);
            return;
        }

        let width = self.width;
        let height = self.height;

        let layer = match self.layers.entry(layer_id) {
            Entry::Vacant(entry) => {
                let mut buf = OptimizedBuffer::new(width, height);
                // Layer buffers must start fully transparent (no tint/no-op cells).
                buf.clear_transparent_with_pool(&mut self.grapheme_pool);
                entry.insert(buf)
            }
            Entry::Occupied(entry) => entry.into_mut(),
        };
        if layer.size() != (width, height) {
            layer.resize_with_pool(&mut self.grapheme_pool, width, height);
            layer.clear_transparent_with_pool(&mut self.grapheme_pool);
        }

        render_fn(layer);
        self.layers_dirty = true;
    }

    /// Return the number of currently allocated overlay layers.
    #[must_use]
    pub fn get_layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Composite all active layers into the base back buffer.
    ///
    /// Higher layer IDs are composited on top of lower ones, using proper alpha blending.
    pub fn merge_layers(&mut self) {
        if !self.layers_dirty {
            self.active_hit_layer = 0;
            return;
        }

        for layer in self.layers.values() {
            self.back_buffer
                .draw_buffer_with_pool(&mut self.grapheme_pool, 0, 0, layer);
        }

        for grid in self.layer_hit_grids.values() {
            self.back_hit_grid.overlay(grid);
        }

        self.layers_dirty = false;
        self.active_hit_layer = 0;
    }

    /// Clear the back buffer.
    pub fn clear(&mut self) {
        self.back_buffer
            .clear_with_pool(&mut self.grapheme_pool, self.background);
        self.back_hit_grid.clear();
        self.clear_overlay_layers();
    }

    /// Present the back buffer to screen (swap buffers).
    pub fn present(&mut self) -> io::Result<()> {
        if self.layers_dirty {
            self.merge_layers();
        }
        if self.show_debug_overlay {
            self.draw_debug_overlay();
        }

        let total_cells = (self.width as usize).saturating_mul(self.height as usize);
        // Use cached diff to avoid per-frame allocation
        self.cached_diff
            .compute_into(&self.front_buffer, &self.back_buffer);

        if self.force_redraw || self.cached_diff.should_full_redraw(total_cells) {
            self.present_force()?;
            self.update_stats(total_cells);
            self.force_redraw = false;
        } else {
            self.present_diff()?;
            self.update_stats(self.cached_diff.change_count);
        }

        // Swap buffers
        std::mem::swap(&mut self.front_buffer, &mut self.back_buffer);
        std::mem::swap(&mut self.front_hit_grid, &mut self.back_hit_grid);
        self.back_buffer
            .clear_with_pool(&mut self.grapheme_pool, self.background);
        self.back_hit_grid.clear();
        self.clear_overlay_layers();

        Ok(())
    }

    /// Force a full redraw.
    pub fn present_force(&mut self) -> io::Result<()> {
        if self.terminal.capabilities().sync_output {
            self.terminal.begin_sync()?;
        }

        self.scratch_buffer.clear();
        let mut writer = AnsiWriter::new(&mut self.scratch_buffer);
        // Emit cursor home to synchronize terminal cursor with writer's internal tracking.
        // The writer starts tracking at (0,0), but the terminal cursor may be elsewhere
        // (e.g., pending-wrap state at end of previous frame).
        writer.write_str("\x1b[H");

        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(cell) = self.back_buffer.get(x, y) {
                    if !cell.is_continuation() {
                        // Always move cursor to exact position before writing
                        // This ensures correct positioning even when cells are skipped
                        writer.move_cursor(y, x);
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
    fn present_diff(&mut self) -> io::Result<()> {
        if self.terminal.capabilities().sync_output {
            self.terminal.begin_sync()?;
        }

        self.scratch_buffer.clear();
        let mut writer = AnsiWriter::new(&mut self.scratch_buffer);
        // Emit cursor home to synchronize terminal cursor with writer's internal tracking.
        // The writer starts tracking at (0,0), but the terminal cursor may be elsewhere
        // from the previous frame. Without this, relative moves would be incorrect.
        writer.write_str("\x1b[H");

        for region in &self.cached_diff.dirty_regions {
            for i in 0..region.width {
                let x = region.x + i;
                let y = region.y;
                let back_cell = self.back_buffer.get(x, y);
                if let Some(cell) = back_cell {
                    // Skip continuation cells - they don't produce output
                    if cell.is_continuation() {
                        continue;
                    }
                    // Always move cursor to exact position before writing
                    // This ensures correct positioning even when continuation cells are skipped
                    writer.move_cursor(y, x);
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
        self.front_hit_grid = HitGrid::new(width, height);
        self.back_hit_grid = HitGrid::new(width, height);
        self.resize_overlay_layers(width, height);
        self.hit_scissor.clear();
        // Clear cached diff (it will grow as needed on next present)
        self.cached_diff.clear();
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
                let hit_grid = if self.active_hit_layer == 0 {
                    &mut self.back_hit_grid
                } else {
                    let width = self.width;
                    let height = self.height;
                    let grid = self
                        .layer_hit_grids
                        .entry(self.active_hit_layer)
                        .or_insert_with(|| HitGrid::new(width, height));
                    if grid.size() != (width, height) {
                        grid.resize(width, height);
                    }
                    self.layers_dirty = true;
                    grid
                };

                hit_grid.register(
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
        self.front_hit_grid.test(x, y)
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
        let hitgrid_bytes = self.front_hit_grid.byte_size() + self.back_hit_grid.byte_size();
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

    fn clear_overlay_layers(&mut self) {
        for layer in self.layers.values_mut() {
            layer.clear_transparent_with_pool(&mut self.grapheme_pool);
        }
        for grid in self.layer_hit_grids.values_mut() {
            grid.clear();
        }
        self.active_hit_layer = 0;
        self.layers_dirty = false;
    }

    fn resize_overlay_layers(&mut self, width: u32, height: u32) {
        for layer in self.layers.values_mut() {
            layer.resize_with_pool(&mut self.grapheme_pool, width, height);
            layer.clear_transparent_with_pool(&mut self.grapheme_pool);
        }
        for grid in self.layer_hit_grids.values_mut() {
            grid.resize(width, height);
        }
        self.active_hit_layer = 0;
        self.layers_dirty = false;
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact float comparison is intentional in tests
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
        // Zero dimensions are clamped to 1 to prevent division by zero in iter_cells
        let buf = OptimizedBuffer::new(0, 0);
        assert_eq!(buf.size(), (1, 1));
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
        buf.set(5, 5, cell);
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

    // ============================================
    // Renderer Constructor & Lifecycle Tests
    // ============================================
    //
    // These tests create actual Renderer instances with all terminal features
    // disabled (no alt screen, cursor hiding, mouse, or capability queries).
    // This allows testing Renderer logic without requiring a real terminal.

    /// Create a test renderer with all terminal options disabled.
    fn test_renderer(width: u32, height: u32) -> Renderer {
        Renderer::new_with_options(
            width,
            height,
            RendererOptions {
                use_alt_screen: false,
                hide_cursor: false,
                enable_mouse: false,
                query_capabilities: false,
            },
        )
        .expect("test renderer creation should succeed with disabled options")
    }

    /// Commit pending hit registrations so they are visible via `hit_test()`.
    ///
    /// Renderer hit testing is based on the last *presented* frame. During a frame,
    /// hit registrations are accumulated separately and promoted after present.
    fn commit_hits_for_test(r: &mut Renderer) {
        if r.layers_dirty {
            r.merge_layers();
        }
        std::mem::swap(&mut r.front_hit_grid, &mut r.back_hit_grid);
        r.back_hit_grid.clear();
    }

    // --- new() / new_with_options() ---

    #[test]
    fn test_renderer_new_with_all_options_disabled() {
        let result = Renderer::new_with_options(
            80,
            24,
            RendererOptions {
                use_alt_screen: false,
                hide_cursor: false,
                enable_mouse: false,
                query_capabilities: false,
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_renderer_new_stores_dimensions() {
        let r = test_renderer(80, 24);
        assert_eq!(r.size(), (80, 24));
    }

    #[test]
    fn test_renderer_new_creates_matching_back_buffer() {
        let mut r = test_renderer(80, 24);
        assert_eq!(r.buffer().size(), (80, 24));
    }

    #[test]
    fn test_renderer_new_creates_matching_front_buffer() {
        let r = test_renderer(80, 24);
        assert_eq!(r.front_buffer().size(), (80, 24));
    }

    #[test]
    fn test_renderer_new_various_dimensions() {
        for &(w, h) in &[(1, 1), (10, 10), (200, 50), (1, 100), (100, 1)] {
            let r = test_renderer(w, h);
            assert_eq!(r.size(), (w, h), "Failed for dimensions ({w}, {h})");
        }
    }

    #[test]
    fn test_renderer_new_initializes_zero_stats() {
        let r = test_renderer(80, 24);
        let stats = r.stats();
        assert_eq!(stats.frames, 0);
        assert_eq!(stats.last_frame_cells, 0);
        assert_eq!(stats.fps, 0.0);
    }

    #[test]
    fn test_renderer_new_default_background_black() {
        let mut r = test_renderer(10, 10);
        // clear() uses the background color, which defaults to black
        r.clear();
        let cell = r.buffer().get(0, 0).unwrap();
        assert_eq!(cell.bg, Rgba::BLACK);
    }

    #[test]
    fn test_renderer_new_single_cell() {
        let r = test_renderer(1, 1);
        assert_eq!(r.size(), (1, 1));
        assert!(r.front_buffer().get(0, 0).is_some());
    }

    // --- resize() ---

    #[test]
    fn test_resize_changes_reported_dimensions() {
        let mut r = test_renderer(80, 24);
        r.resize(100, 50).unwrap();
        assert_eq!(r.size(), (100, 50));
    }

    #[test]
    fn test_resize_changes_back_buffer_size() {
        let mut r = test_renderer(80, 24);
        r.resize(100, 50).unwrap();
        assert_eq!(r.buffer().size(), (100, 50));
    }

    #[test]
    fn test_resize_changes_front_buffer_size() {
        let mut r = test_renderer(80, 24);
        r.resize(100, 50).unwrap();
        assert_eq!(r.front_buffer().size(), (100, 50));
    }

    #[test]
    fn test_resize_shrink() {
        let mut r = test_renderer(80, 24);
        r.resize(40, 12).unwrap();
        assert_eq!(r.size(), (40, 12));
        assert_eq!(r.buffer().size(), (40, 12));
    }

    #[test]
    fn test_resize_grow() {
        let mut r = test_renderer(40, 12);
        r.resize(160, 48).unwrap();
        assert_eq!(r.size(), (160, 48));
        assert_eq!(r.buffer().size(), (160, 48));
    }

    #[test]
    fn test_resize_to_single_cell() {
        let mut r = test_renderer(80, 24);
        r.resize(1, 1).unwrap();
        assert_eq!(r.size(), (1, 1));
    }

    #[test]
    fn test_resize_sets_force_redraw() {
        // After resize, the next present should be a full redraw.
        // We verify this indirectly: present after resize should report
        // total cells (full redraw), not a partial diff count.
        let mut r = test_renderer(10, 10);
        // First present clears the initial force_redraw flag
        r.present().unwrap();

        // Resize
        r.resize(20, 20).unwrap();

        // Present after resize should be a full redraw (400 cells)
        r.present().unwrap();
        assert_eq!(r.stats().last_frame_cells, 400);
    }

    #[test]
    fn test_resize_clears_hit_grid() {
        let mut r = test_renderer(80, 24);
        r.register_hit_area(10, 5, 20, 3, 42);
        commit_hits_for_test(&mut r);
        assert_eq!(r.hit_test(15, 6), Some(42));

        r.resize(80, 24).unwrap();
        // Hit grid is rebuilt on resize
        assert_eq!(r.hit_test(15, 6), None);
    }

    // --- invalidate() ---

    #[test]
    fn test_invalidate_does_not_panic() {
        let mut r = test_renderer(80, 24);
        r.invalidate();
    }

    #[test]
    fn test_invalidate_multiple_times() {
        let mut r = test_renderer(80, 24);
        r.invalidate();
        r.invalidate();
        r.invalidate();
        // Should not panic or cause issues
    }

    #[test]
    fn test_invalidate_forces_full_redraw() {
        let mut r = test_renderer(10, 10);
        // First present: full redraw (force_redraw starts true)
        r.present().unwrap();
        assert_eq!(r.stats().last_frame_cells, 100);

        // Invalidate and present again
        r.invalidate();
        r.present().unwrap();
        // Should be a full redraw (100 cells for 10x10)
        assert_eq!(r.stats().last_frame_cells, 100);
    }

    // --- cleanup() ---

    #[test]
    fn test_cleanup_succeeds() {
        let mut r = test_renderer(80, 24);
        assert!(r.cleanup().is_ok());
    }

    #[test]
    fn test_cleanup_idempotent() {
        let mut r = test_renderer(80, 24);
        r.cleanup().unwrap();
        r.cleanup().unwrap();
        // Safe to call multiple times
    }

    #[test]
    fn test_cleanup_on_fresh_renderer() {
        // cleanup should work even on a renderer that never drew anything
        let mut r = test_renderer(80, 24);
        assert!(r.cleanup().is_ok());
    }

    // --- Buffer access through Renderer ---

    #[test]
    fn test_renderer_buffer_is_writable() {
        let mut r = test_renderer(10, 10);
        r.buffer()
            .set(5, 5, Cell::new('X', crate::style::Style::NONE));
        let cell = r.buffer().get(5, 5).unwrap();
        assert!(matches!(cell.content, crate::cell::CellContent::Char('X')));
    }

    #[test]
    fn test_renderer_buffer_with_pool() {
        let mut r = test_renderer(10, 10);
        let (buf, pool) = r.buffer_with_pool();
        let id = pool.alloc("👨‍👩‍👧");
        assert!(pool.get(id).is_some());
        assert_eq!(buf.size(), (10, 10));
    }

    #[test]
    fn test_renderer_front_buffer_readable() {
        let r = test_renderer(10, 10);
        let front = r.front_buffer();
        // Front buffer should exist and be accessible
        assert!(front.get(0, 0).is_some());
        assert_eq!(front.size(), (10, 10));
    }

    // --- set_background() ---

    #[test]
    fn test_set_background_affects_clear() {
        let mut r = test_renderer(10, 10);
        r.set_background(Rgba::RED);
        r.clear();
        let cell = r.buffer().get(0, 0).unwrap();
        assert_eq!(cell.bg, Rgba::RED);
    }

    #[test]
    fn test_set_background_multiple_colors() {
        let mut r = test_renderer(10, 10);
        for color in [Rgba::RED, Rgba::GREEN, Rgba::BLUE, Rgba::WHITE] {
            r.set_background(color);
            r.clear();
            let cell = r.buffer().get(0, 0).unwrap();
            assert_eq!(cell.bg, color);
        }
    }

    // --- set_debug_overlay() ---

    #[test]
    fn test_set_debug_overlay_toggle() {
        let mut r = test_renderer(80, 24);
        r.set_debug_overlay(true);
        r.set_debug_overlay(false);
        r.set_debug_overlay(true);
        // Should not panic
    }

    // --- Capabilities access ---

    #[test]
    fn test_renderer_capabilities_accessible() {
        let r = test_renderer(80, 24);
        let caps = r.capabilities();
        // Just verify we can access capabilities without panic
        let _ = caps.hyperlinks;
        let _ = caps.sync_output;
    }

    #[test]
    fn test_renderer_capabilities_mut_writable() {
        let mut r = test_renderer(80, 24);
        r.capabilities_mut().hyperlinks = true;
        assert!(r.capabilities().hyperlinks);
        r.capabilities_mut().hyperlinks = false;
        assert!(!r.capabilities().hyperlinks);
    }

    // --- Pool access ---

    #[test]
    fn test_renderer_link_pool_usable() {
        let mut r = test_renderer(80, 24);
        let id = r.link_pool().alloc("https://example.com");
        assert_eq!(r.link_pool().get(id), Some("https://example.com"));
    }

    #[test]
    fn test_renderer_grapheme_pool_usable() {
        let mut r = test_renderer(80, 24);
        let id = r.grapheme_pool().alloc("test_grapheme");
        assert_eq!(r.grapheme_pool().get(id), Some("test_grapheme"));
    }

    #[test]
    fn test_renderer_grapheme_pool_ref_readable() {
        let r = test_renderer(80, 24);
        let _pool = r.grapheme_pool_ref();
        // Pool exists and is readable without panic
    }

    // --- Hit testing through Renderer ---

    #[test]
    fn test_renderer_register_and_hit_test() {
        let mut r = test_renderer(80, 24);
        r.register_hit_area(10, 5, 20, 3, 42);
        commit_hits_for_test(&mut r);
        assert_eq!(r.hit_test(15, 6), Some(42));
        assert_eq!(r.hit_test(5, 3), None);
    }

    #[test]
    fn test_renderer_hit_scissor_clips_registration() {
        let mut r = test_renderer(80, 24);
        // Push scissor that restricts to a sub-region
        r.push_hit_scissor(ClipRect::new(10, 10, 20, 20));
        // Register a full-screen hit area
        r.register_hit_area(0, 0, 80, 24, 1);
        commit_hits_for_test(&mut r);
        // Inside scissor should hit
        assert_eq!(r.hit_test(15, 15), Some(1));
        // Outside scissor should miss
        assert_eq!(r.hit_test(5, 5), None);
        r.pop_hit_scissor();
    }

    #[test]
    fn test_renderer_clear_hit_scissors_restores_full_area() {
        let mut r = test_renderer(80, 24);
        r.push_hit_scissor(ClipRect::new(10, 10, 5, 5));
        r.clear_hit_scissors();
        r.register_hit_area(0, 0, 80, 24, 1);
        commit_hits_for_test(&mut r);
        // After clearing scissors, full area should be accessible
        assert_eq!(r.hit_test(5, 5), Some(1));
    }

    // --- Present integration ---

    #[test]
    fn test_present_succeeds_on_fresh_renderer() {
        let mut r = test_renderer(80, 24);
        assert!(r.present().is_ok());
    }

    #[test]
    fn test_present_increments_frame_count() {
        let mut r = test_renderer(10, 10);
        r.present().unwrap();
        assert_eq!(r.stats().frames, 1);
        r.present().unwrap();
        assert_eq!(r.stats().frames, 2);
        r.present().unwrap();
        assert_eq!(r.stats().frames, 3);
    }

    #[test]
    fn test_present_updates_stats() {
        let mut r = test_renderer(10, 10);
        r.buffer()
            .set(0, 0, Cell::new('X', crate::style::Style::NONE));
        r.present().unwrap();
        assert!(r.stats().frames > 0);
        assert!(r.stats().buffer_bytes > 0);
        assert!(r.stats().total_bytes > 0);
    }

    #[test]
    fn test_present_force_succeeds() {
        let mut r = test_renderer(10, 10);
        assert!(r.present_force().is_ok());
    }

    #[test]
    fn test_present_swaps_buffers() {
        let mut r = test_renderer(10, 10);
        // Draw to back buffer
        r.buffer()
            .set(5, 5, Cell::new('A', crate::style::Style::NONE));

        // Before present, front should not have 'A'
        let front_cell = r.front_buffer().get(5, 5).unwrap();
        assert!(!matches!(
            front_cell.content,
            crate::cell::CellContent::Char('A')
        ));

        // Present swaps buffers
        r.present().unwrap();

        // After present, front should have 'A' (what was back)
        let front_cell = r.front_buffer().get(5, 5).unwrap();
        assert!(matches!(
            front_cell.content,
            crate::cell::CellContent::Char('A')
        ));
    }

    #[test]
    fn test_present_clears_back_buffer_after_swap() {
        let mut r = test_renderer(10, 10);
        r.buffer()
            .set(5, 5, Cell::new('Z', crate::style::Style::NONE));
        r.present().unwrap();

        // After present, back buffer should be cleared
        let back_cell = r.buffer().get(5, 5).unwrap();
        assert!(
            !matches!(back_cell.content, crate::cell::CellContent::Char('Z')),
            "Back buffer should be cleared after present"
        );
    }

    // --- Clear through Renderer ---

    #[test]
    fn test_renderer_clear_resets_buffer() {
        let mut r = test_renderer(10, 10);
        r.buffer()
            .set(0, 0, Cell::new('X', crate::style::Style::NONE));
        r.clear();
        let cell = r.buffer().get(0, 0).unwrap();
        assert_eq!(cell.bg, Rgba::BLACK);
    }

    #[test]
    fn test_renderer_clear_resets_hit_grid() {
        let mut r = test_renderer(80, 24);
        r.register_hit_area(0, 0, 80, 24, 1);
        commit_hits_for_test(&mut r);
        assert_eq!(r.hit_test(5, 5), Some(1));

        r.clear();
        commit_hits_for_test(&mut r);
        assert_eq!(r.hit_test(5, 5), None);
    }

    // ============================================
    // Renderer Hit Testing & Scissor Tests (bd-aj8c)
    // ============================================

    #[test]
    fn test_renderer_multiple_non_overlapping_hit_areas() {
        let mut r = test_renderer(80, 24);
        r.register_hit_area(0, 0, 10, 5, 1);
        r.register_hit_area(20, 0, 10, 5, 2);
        r.register_hit_area(40, 0, 10, 5, 3);
        commit_hits_for_test(&mut r);

        assert_eq!(r.hit_test(5, 2), Some(1));
        assert_eq!(r.hit_test(25, 2), Some(2));
        assert_eq!(r.hit_test(45, 2), Some(3));
        assert_eq!(r.hit_test(15, 2), None); // gap
    }

    #[test]
    fn test_renderer_overlapping_hit_areas_later_wins() {
        let mut r = test_renderer(80, 24);
        r.register_hit_area(0, 0, 30, 10, 100);
        r.register_hit_area(20, 0, 30, 10, 200);
        commit_hits_for_test(&mut r);

        // Overlap region: later registration wins
        assert_eq!(r.hit_test(5, 5), Some(100)); // Only in first
        assert_eq!(r.hit_test(25, 5), Some(200)); // Overlap, second wins
        assert_eq!(r.hit_test(45, 5), Some(200)); // Only in second
    }

    #[test]
    fn test_renderer_hit_test_boundary_conditions() {
        let mut r = test_renderer(80, 24);
        r.register_hit_area(10, 5, 20, 10, 1);
        commit_hits_for_test(&mut r);

        // Exact boundaries
        assert_eq!(r.hit_test(10, 5), Some(1)); // Top-left
        assert_eq!(r.hit_test(29, 5), Some(1)); // Top-right
        assert_eq!(r.hit_test(10, 14), Some(1)); // Bottom-left
        assert_eq!(r.hit_test(29, 14), Some(1)); // Bottom-right

        // Just outside
        assert_eq!(r.hit_test(9, 5), None);
        assert_eq!(r.hit_test(30, 5), None);
        assert_eq!(r.hit_test(10, 4), None);
        assert_eq!(r.hit_test(10, 15), None);
    }

    #[test]
    fn test_renderer_hit_area_outside_buffer_bounds() {
        let mut r = test_renderer(20, 20);
        // Register area that extends beyond buffer
        r.register_hit_area(15, 15, 20, 20, 1);
        commit_hits_for_test(&mut r);

        // Inside buffer + area
        assert_eq!(r.hit_test(18, 18), Some(1));
        // Outside buffer entirely
        assert_eq!(r.hit_test(25, 25), None);
    }

    #[test]
    fn test_renderer_nested_hit_scissors() {
        let mut r = test_renderer(80, 24);

        // Outer scissor
        r.push_hit_scissor(ClipRect::new(5, 5, 40, 20));
        // Inner scissor (narrower)
        r.push_hit_scissor(ClipRect::new(10, 8, 20, 10));

        // Register hit area that spans beyond both scissors
        r.register_hit_area(0, 0, 80, 24, 1);
        commit_hits_for_test(&mut r);

        // Only the innermost scissor intersection should register
        assert_eq!(r.hit_test(15, 12), Some(1)); // Inside inner
        assert_eq!(r.hit_test(7, 7), None); // Inside outer but outside inner
        assert_eq!(r.hit_test(2, 2), None); // Outside both

        r.pop_hit_scissor(); // Pop inner
        r.pop_hit_scissor(); // Pop outer
    }

    #[test]
    fn test_renderer_pop_hit_scissor_restores_previous() {
        let mut r = test_renderer(80, 24);

        // Push restrictive scissor
        r.push_hit_scissor(ClipRect::new(10, 10, 10, 10));
        r.register_hit_area(0, 0, 80, 24, 1);
        commit_hits_for_test(&mut r);
        assert_eq!(r.hit_test(15, 15), Some(1));
        assert_eq!(r.hit_test(5, 5), None); // Clipped

        // Pop scissor
        r.pop_hit_scissor();

        // After pop, full area should be available for new registrations
        r.register_hit_area(0, 0, 80, 24, 2);
        commit_hits_for_test(&mut r);
        assert_eq!(r.hit_test(5, 5), Some(2));
    }

    #[test]
    fn test_renderer_hit_test_after_present() {
        let mut r = test_renderer(80, 24);
        r.register_hit_area(10, 5, 20, 3, 42);
        r.buffer()
            .draw_text(0, 0, "Hello", crate::style::Style::NONE);
        r.present().unwrap();

        // Hit testing is based on the last presented frame.
        assert_eq!(r.hit_test(15, 6), Some(42));

        // Next present without re-registration should clear the hit grid for the new frame.
        r.present().unwrap();
        assert_eq!(r.hit_test(15, 6), None);
    }

    #[test]
    fn test_renderer_hit_test_none_on_empty() {
        let r = test_renderer(80, 24);
        // No areas registered
        assert_eq!(r.hit_test(0, 0), None);
        assert_eq!(r.hit_test(40, 12), None);
        assert_eq!(r.hit_test(79, 23), None);
    }

    // ============================================
    // Layered Rendering Tests (bd-1scf, bd-6i8r, bd-flgw)
    // ============================================

    #[test]
    fn test_get_layer_count_tracks_allocated_layers() {
        let mut r = test_renderer(10, 10);
        assert_eq!(r.get_layer_count(), 0);

        r.render_to_layer(1, |_| {});
        assert_eq!(r.get_layer_count(), 1);

        r.render_to_layer(5, |_| {});
        assert_eq!(r.get_layer_count(), 2);

        // Re-using an existing layer should not increase the count.
        r.render_to_layer(1, |_| {});
        assert_eq!(r.get_layer_count(), 2);
    }

    #[test]
    fn test_merge_layers_composites_higher_layers_on_top() {
        let mut r = test_renderer(3, 1);

        r.buffer()
            .set(0, 0, Cell::new('A', crate::style::Style::NONE));
        r.buffer()
            .set(1, 0, Cell::new('X', crate::style::Style::NONE));

        r.render_to_layer(1, |buf| {
            buf.set(0, 0, Cell::new('B', crate::style::Style::NONE));
        });
        r.render_to_layer(2, |buf| {
            buf.set(0, 0, Cell::new('C', crate::style::Style::NONE));
        });

        r.merge_layers();

        let top = r.buffer().get(0, 0).unwrap();
        assert!(matches!(top.content, crate::cell::CellContent::Char('C')));

        // Unaffected cells remain from the base layer.
        let base_only = r.buffer().get(1, 0).unwrap();
        assert!(matches!(
            base_only.content,
            crate::cell::CellContent::Char('X')
        ));
    }

    #[test]
    fn test_merge_layers_does_not_tint_base_fg_when_layer_is_transparent() {
        let mut r = test_renderer(1, 1);
        r.buffer().set(
            0,
            0,
            Cell::new(
                'A',
                crate::style::Style::builder()
                    .fg(Rgba::RED)
                    .bg(Rgba::BLACK)
                    .build(),
            ),
        );

        // Create an overlay layer but do not draw into it.
        r.render_to_layer(1, |_| {});
        r.merge_layers();

        let cell = r.buffer().get(0, 0).unwrap();
        assert_eq!(cell.fg, Rgba::RED);
        assert_eq!(cell.bg, Rgba::BLACK);
    }

    #[test]
    fn test_merge_layers_composites_hit_grids_by_layer_id() {
        let mut r = test_renderer(10, 10);

        // Register a hit in an overlay layer first...
        r.render_to_layer(1, |_| {});
        r.register_hit_area(0, 0, 1, 1, 111);

        // ...then register a base hit after. Base registration order should not
        // override a higher layer when the grids are merged.
        r.render_to_layer(0, |_| {});
        r.register_hit_area(0, 0, 1, 1, 222);
        r.register_hit_area(1, 0, 1, 1, 333);

        r.merge_layers();
        commit_hits_for_test(&mut r);

        // Overlay wins where it has a hit id.
        assert_eq!(r.hit_test(0, 0), Some(111));
        // Click-through works where the overlay has no hit id.
        assert_eq!(r.hit_test(1, 0), Some(333));
    }

    #[test]
    fn test_present_merges_layers_automatically() {
        let mut r = test_renderer(2, 1);
        r.buffer()
            .set(0, 0, Cell::new('A', crate::style::Style::NONE));

        r.render_to_layer(1, |buf| {
            buf.set(0, 0, Cell::new('B', crate::style::Style::NONE));
        });

        r.present().unwrap();

        let cell = r.front_buffer().get(0, 0).unwrap();
        assert!(matches!(cell.content, crate::cell::CellContent::Char('B')));
    }

    // ============================================
    // Renderer Terminal Control Tests (bd-1303)
    // ============================================

    #[test]
    fn test_set_cursor_visible() {
        let mut r = test_renderer(80, 24);
        // Show cursor at position - writes ANSI to stdout
        assert!(r.set_cursor(10, 5, true).is_ok());
    }

    #[test]
    fn test_set_cursor_hidden() {
        let mut r = test_renderer(80, 24);
        // Hide cursor - writes ANSI to stdout
        assert!(r.set_cursor(0, 0, false).is_ok());
    }

    #[test]
    fn test_set_cursor_at_origin() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_cursor(0, 0, true).is_ok());
    }

    #[test]
    fn test_set_cursor_at_boundary() {
        let mut r = test_renderer(80, 24);
        // Cursor beyond buffer - should not panic, ANSI output is unbounded
        assert!(r.set_cursor(79, 23, true).is_ok());
    }

    #[test]
    fn test_set_cursor_style_block() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_cursor_style(CursorStyle::Block, false).is_ok());
        assert!(r.set_cursor_style(CursorStyle::Block, true).is_ok());
    }

    #[test]
    fn test_set_cursor_style_underline() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_cursor_style(CursorStyle::Underline, false).is_ok());
        assert!(r.set_cursor_style(CursorStyle::Underline, true).is_ok());
    }

    #[test]
    fn test_set_cursor_style_bar() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_cursor_style(CursorStyle::Bar, false).is_ok());
        assert!(r.set_cursor_style(CursorStyle::Bar, true).is_ok());
    }

    #[test]
    fn test_set_title_basic() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_title("OpenTUI").is_ok());
    }

    #[test]
    fn test_set_title_empty() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_title("").is_ok());
    }

    #[test]
    fn test_set_title_special_characters() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_title("Hello — World 🌍").is_ok());
    }

    #[test]
    fn test_set_title_with_unicode() {
        let mut r = test_renderer(80, 24);
        assert!(r.set_title("日本語タイトル").is_ok());
    }

    #[test]
    fn test_capabilities_color_support_readable() {
        let r = test_renderer(80, 24);
        let caps = r.capabilities();
        // Color support should be some value (depends on env)
        let _ = caps.color;
        let _ = caps.unicode;
        let _ = caps.mouse;
    }

    #[test]
    fn test_capabilities_override_persists() {
        let mut r = test_renderer(80, 24);
        r.capabilities_mut().sync_output = true;
        assert!(r.capabilities().sync_output);
        r.capabilities_mut().sync_output = false;
        assert!(!r.capabilities().sync_output);
    }

    #[test]
    fn test_capabilities_hyperlinks_override() {
        let mut r = test_renderer(80, 24);
        let original = r.capabilities().hyperlinks;
        r.capabilities_mut().hyperlinks = !original;
        assert_eq!(r.capabilities().hyperlinks, !original);
    }

    #[test]
    fn test_set_background_and_present() {
        let mut r = test_renderer(10, 10);
        r.set_background(Rgba::BLUE);
        r.clear();
        // Verify the buffer has the new background
        let cell = r.buffer().get(5, 5).unwrap();
        assert_eq!(cell.bg, Rgba::BLUE);
        // Present should succeed with new background
        assert!(r.present().is_ok());
    }
}
