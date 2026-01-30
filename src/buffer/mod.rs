//! Cell-based frame buffer with alpha blending and scissoring.
//!
//! This module provides [`OptimizedBuffer`], the primary drawing surface for
//! terminal rendering. Buffers are 2D grids of cells that support:
//!
//! - **Basic drawing**: Set individual cells, draw text, draw boxes
//! - **Scissor clipping**: Restrict drawing to rectangular regions
//! - **Opacity stacking**: Apply transparency to groups of operations
//! - **Alpha blending**: Composite cells using Porter-Duff "over"
//! - **Buffer compositing**: Draw one buffer onto another
//!
//! # Examples
//!
//! ```
//! use opentui::{OptimizedBuffer, Style, Rgba, Cell};
//! use opentui::buffer::ClipRect;
//!
//! let mut buf = OptimizedBuffer::new(80, 24);
//!
//! // Clear with background
//! buf.clear(Rgba::BLACK);
//!
//! // Draw styled text
//! buf.draw_text(10, 5, "Hello!", Style::fg(Rgba::GREEN));
//!
//! // Use scissor to clip drawing
//! buf.push_scissor(ClipRect::new(0, 0, 40, 12));
//! buf.draw_text(0, 0, "This text is clipped to left half", Style::NONE);
//! buf.pop_scissor();
//!
//! // Use opacity for transparent overlays
//! buf.push_opacity(0.5);
//! buf.fill_rect(20, 10, 40, 5, Rgba::BLUE);
//! buf.pop_opacity();
//! ```

// Buffer operations naturally have many parameters for region copying
#![allow(clippy::too_many_arguments)]

mod drawing;
mod opacity;
mod pixel;
mod scissor;

pub use drawing::{BoxOptions, BoxSides, BoxStyle, TitleAlign};
pub use opacity::OpacityStack;
pub use pixel::{GrayscaleBuffer, PixelBuffer};
pub use scissor::{ClipRect, ScissorStack};

use crate::cell::{Cell, CellContent, GraphemeId};
use crate::color::Rgba;
use crate::grapheme_pool::GraphemePool;
use crate::style::Style;
use crate::text::{EditorView, TextBufferView};

/// Optimized cell buffer for terminal rendering.
///
/// The buffer maintains a 2D grid of [`Cell`]s along with scissor and opacity
/// stacks for controlling how drawing operations are applied.
///
/// # Coordinate System
///
/// Coordinates are (x, y) where (0, 0) is the top-left corner. X increases
/// to the right, Y increases downward.
///
/// # Drawing Behavior
///
/// All drawing operations respect the current scissor stack (clipping) and
/// opacity stack (transparency). Use [`set_blended`](Self::set_blended) for
/// alpha-compositing or [`set`](Self::set) for direct replacement.
#[derive(Clone, Debug)]
pub struct OptimizedBuffer {
    width: u32,
    height: u32,
    cells: Vec<Cell>,

    scissor_stack: ScissorStack,
    opacity_stack: OpacityStack,

    id: String,
    respect_alpha: bool,

    /// Grapheme IDs that were overwritten by non-pool operations.
    /// These need to be cleaned up when a pool becomes available.
    orphaned_graphemes: Vec<GraphemeId>,
}

impl OptimizedBuffer {
    /// Create a new buffer with the given dimensions.
    ///
    /// Uses saturating multiplication to prevent overflow for extremely large dimensions.
    /// Zero dimensions are clamped to 1 to prevent division by zero in iteration.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        // Clamp to minimum of 1 to prevent division by zero in iter_cells()
        let width = width.max(1);
        let height = height.max(1);
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            cells: vec![Cell::clear(Rgba::TRANSPARENT); size],
            scissor_stack: ScissorStack::new(),
            opacity_stack: OpacityStack::new(),
            id: String::new(),
            respect_alpha: true,
            orphaned_graphemes: Vec::new(),
        }
    }

    /// Create a named buffer.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Get buffer dimensions.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get buffer width.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get buffer height.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get buffer ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Estimated byte size of the buffer cell storage.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.cells.len() * std::mem::size_of::<Cell>()
    }

    /// Compute cell index with overflow protection.
    ///
    /// Returns `None` if:
    /// - Coordinates are out of bounds
    /// - Index calculation would overflow
    #[inline]
    fn cell_index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        // Use checked arithmetic to prevent overflow on large dimensions
        let row_offset = (y as usize).checked_mul(self.width as usize)?;
        let idx = row_offset.checked_add(x as usize)?;
        // Bounds check (should always pass given x/y bounds, but defense in depth)
        if idx < self.cells.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Get cell at position.
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> Option<&Cell> {
        self.cell_index(x, y).map(|idx| &self.cells[idx])
    }

    /// Get mutable cell at position.
    pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut Cell> {
        self.cell_index(x, y).map(|idx| &mut self.cells[idx])
    }

    /// Set cell at position, respecting scissor and opacity.
    ///
    /// Note: If the cell being overwritten contains a pooled grapheme, the
    /// grapheme ID is tracked for later cleanup via [`Self::clear_with_pool`] or
    /// [`Self::set_with_pool`].
    pub fn set(&mut self, x: u32, y: u32, mut cell: Cell) {
        if !self.is_visible(x, y) {
            return;
        }

        let opacity = self.opacity_stack.current();
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        // Use index to avoid double mutable borrow
        if let Some(idx) = self.cell_index(x, y) {
            // Track orphaned graphemes for later cleanup
            if let CellContent::Grapheme(id) = self.cells[idx].content {
                if id.pool_id() != 0 {
                    self.orphaned_graphemes.push(id);
                }
            }
            self.cells[idx] = cell;
        }
    }

    /// Set cell at position, updating grapheme pool reference counts.
    ///
    /// Also releases any orphaned graphemes from prior non-pool operations.
    pub fn set_with_pool(&mut self, pool: &mut GraphemePool, x: u32, y: u32, mut cell: Cell) {
        // First, release any orphaned graphemes from non-pool operations
        self.drain_orphaned_graphemes(pool);

        if !self.is_visible(x, y) {
            return;
        }

        let opacity = self.opacity_stack.current();
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        if let Some(dest) = self.get_mut(x, y) {
            let old_content = dest.content;
            let new_content = cell.content;

            if old_content != new_content {
                if let CellContent::Grapheme(id) = old_content {
                    if id.pool_id() != 0 {
                        pool.decref(id);
                    }
                }
            } else if let CellContent::Grapheme(id) = new_content {
                // Cancel prior incref from pool.intern() for same-id overwrite.
                if id.pool_id() != 0 {
                    pool.decref(id);
                }
            }

            *dest = cell;
        }
    }

    /// Set cell with alpha blending over existing content.
    ///
    /// Note: If the cell being overwritten contains a pooled grapheme, the
    /// grapheme ID is tracked for later cleanup via [`Self::clear_with_pool`] or
    /// [`Self::set_blended_with_pool`].
    pub fn set_blended(&mut self, x: u32, y: u32, mut cell: Cell) {
        if !self.is_visible(x, y) {
            return;
        }

        let opacity = self.opacity_stack.current();
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        let respect_alpha = self.respect_alpha;
        // Use index to avoid double mutable borrow
        if let Some(idx) = self.cell_index(x, y) {
            // Track orphaned graphemes for later cleanup
            if let CellContent::Grapheme(id) = self.cells[idx].content {
                if id.pool_id() != 0 {
                    self.orphaned_graphemes.push(id);
                }
            }
            if respect_alpha {
                self.cells[idx] = cell.blend_over(&self.cells[idx]);
            } else {
                self.cells[idx] = cell;
            }
        }
    }

    /// Set cell with alpha blending over existing content, updating grapheme pool counts.
    ///
    /// Also releases any orphaned graphemes from prior non-pool operations.
    pub fn set_blended_with_pool(
        &mut self,
        pool: &mut GraphemePool,
        x: u32,
        y: u32,
        mut cell: Cell,
    ) {
        // First, release any orphaned graphemes from non-pool operations
        self.drain_orphaned_graphemes(pool);

        if !self.is_visible(x, y) {
            return;
        }

        let opacity = self.opacity_stack.current();
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        let respect_alpha = self.respect_alpha;
        if let Some(dest) = self.get_mut(x, y) {
            let old_content = dest.content;
            let incoming_content = cell.content;
            let new_cell = if respect_alpha {
                cell.blend_over(dest)
            } else {
                cell
            };
            let new_content = new_cell.content;
            let new_from_input = !respect_alpha || !incoming_content.is_empty();

            if old_content != new_content {
                if let CellContent::Grapheme(id) = old_content {
                    if id.pool_id() != 0 {
                        pool.decref(id);
                    }
                }
            } else if new_from_input {
                if let CellContent::Grapheme(id) = new_content {
                    if id.pool_id() != 0 {
                        pool.decref(id);
                    }
                }
            }

            *dest = new_cell;
        }
    }

    /// Check if position is within current scissor rect.
    fn is_visible(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.scissor_stack.contains(x as i32, y as i32)
    }

    /// Release any orphaned graphemes that were overwritten by non-pool operations.
    ///
    /// When `set()` or `set_blended()` overwrites a cell containing a pooled grapheme,
    /// the grapheme ID is tracked but not immediately released (since no pool is
    /// available). This method decrements the reference count for all such orphans.
    ///
    /// Called automatically by pool-aware methods like [`Self::clear_with_pool`] and
    /// [`Self::set_with_pool`].
    pub fn drain_orphaned_graphemes(&mut self, pool: &mut GraphemePool) {
        for id in self.orphaned_graphemes.drain(..) {
            pool.decref(id);
        }
    }

    /// Clear entire buffer with background color.
    pub fn clear(&mut self, bg: Rgba) {
        // Create the clear cell once and fill the entire buffer
        // This is more efficient than creating Cell::clear(bg) per cell
        let clear_cell = Cell::clear(bg);
        self.cells.fill(clear_cell);
    }

    /// Clear entire buffer with background color, updating grapheme pool counts.
    ///
    /// Also releases any orphaned graphemes from prior non-pool operations.
    pub fn clear_with_pool(&mut self, pool: &mut GraphemePool, bg: Rgba) {
        // First, release any orphaned graphemes from non-pool operations
        self.drain_orphaned_graphemes(pool);

        let clear_cell = Cell::clear(bg);
        for cell in &mut self.cells {
            if let CellContent::Grapheme(id) = cell.content {
                if id.pool_id() != 0 {
                    pool.decref(id);
                }
            }
            *cell = clear_cell;
        }
    }

    /// Fill a rectangular region with background color.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, bg: Rgba) {
        if w == 0 || h == 0 || self.width == 0 || self.height == 0 {
            return;
        }

        let mut x0 = x.min(self.width);
        let mut y0 = y.min(self.height);
        let mut x1 = x.saturating_add(w).min(self.width);
        let mut y1 = y.saturating_add(h).min(self.height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let scissor = self.scissor_stack.current();
        if scissor.is_empty() {
            return;
        }

        let scissor_start_x = scissor.x.max(0) as u32;
        let scissor_start_y = scissor.y.max(0) as u32;
        let scissor_end_x = scissor.x.saturating_add_unsigned(scissor.width).max(0) as u32;
        let scissor_end_y = scissor.y.saturating_add_unsigned(scissor.height).max(0) as u32;

        x0 = x0.max(scissor_start_x);
        y0 = y0.max(scissor_start_y);
        x1 = x1.min(scissor_end_x);
        y1 = y1.min(scissor_end_y);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let opacity = self.opacity_stack.current();
        let needs_blend = opacity < 1.0 || !bg.is_opaque();
        let mut cell = Cell::clear(bg);
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        // Optimized path for opaque fill (erasure) or when alpha is disabled
        if !needs_blend || !self.respect_alpha {
            let row_width = self.width as usize;
            for row in y0..y1 {
                let row_start = row as usize * row_width;
                let start = row_start + x0 as usize;
                let end = row_start + x1 as usize;
                self.cells[start..end].fill(cell);
            }
            return;
        }

        // Blending path for transparent fill (overlay/tint)
        let row_width = self.width as usize;
        for row in y0..y1 {
            let row_start = row as usize * row_width;
            for col in x0..x1 {
                let dest_idx = row_start + col as usize;
                let dest_cell = &mut self.cells[dest_idx];
                *dest_cell = cell.blend_over(dest_cell);
            }
        }
    }

    /// Fill a rectangular region with background color, updating grapheme pool counts.
    pub fn fill_rect_with_pool(
        &mut self,
        pool: &mut GraphemePool,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        bg: Rgba,
    ) {
        if w == 0 || h == 0 || self.width == 0 || self.height == 0 {
            return;
        }

        let mut x0 = x.min(self.width);
        let mut y0 = y.min(self.height);
        let mut x1 = x.saturating_add(w).min(self.width);
        let mut y1 = y.saturating_add(h).min(self.height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let scissor = self.scissor_stack.current();
        if scissor.is_empty() {
            return;
        }

        let scissor_start_x = scissor.x.max(0) as u32;
        let scissor_start_y = scissor.y.max(0) as u32;
        let scissor_end_x = scissor.x.saturating_add_unsigned(scissor.width).max(0) as u32;
        let scissor_end_y = scissor.y.saturating_add_unsigned(scissor.height).max(0) as u32;

        x0 = x0.max(scissor_start_x);
        y0 = y0.max(scissor_start_y);
        x1 = x1.min(scissor_end_x);
        y1 = y1.min(scissor_end_y);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let opacity = self.opacity_stack.current();
        let needs_blend = opacity < 1.0 || !bg.is_opaque();
        let mut cell = Cell::clear(bg);
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        let row_width = self.width as usize;

        // Optimized path for opaque fill (erasure) or when alpha is disabled
        if !needs_blend || !self.respect_alpha {
            for row in y0..y1 {
                let row_start = row as usize * row_width;
                for col in x0..x1 {
                    let idx = row_start + col as usize;
                    if let CellContent::Grapheme(id) = self.cells[idx].content {
                        if id.pool_id() != 0 {
                            pool.decref(id);
                        }
                    }
                    self.cells[idx] = cell;
                }
            }
            return;
        }

        // Blending path for transparent fill (overlay/tint)
        for row in y0..y1 {
            let row_start = row as usize * row_width;
            for col in x0..x1 {
                let dest_idx = row_start + col as usize;
                let dest_cell = &mut self.cells[dest_idx];
                let old_content = dest_cell.content;
                let new_cell = cell.blend_over(dest_cell);
                let new_content = new_cell.content;

                if old_content != new_content {
                    if let CellContent::Grapheme(id) = old_content {
                        if id.pool_id() != 0 {
                            pool.decref(id);
                        }
                    }
                }

                *dest_cell = new_cell;
            }
        }
    }

    /// Draw text at position with style.
    ///
    /// **Note:** Multi-codepoint graphemes are stored with placeholder IDs.
    /// For proper grapheme pool integration, use [`Self::draw_text_with_pool`].
    pub fn draw_text(&mut self, x: u32, y: u32, text: &str, style: Style) {
        drawing::draw_text(self, x, y, text, style);
    }

    /// Draw text at position, allocating grapheme IDs from the pool.
    ///
    /// This version properly allocates multi-codepoint graphemes (emoji, ZWJ sequences)
    /// in the pool, allowing them to be resolved during rendering.
    pub fn draw_text_with_pool(
        &mut self,
        pool: &mut crate::grapheme_pool::GraphemePool,
        x: u32,
        y: u32,
        text: &str,
        style: Style,
    ) {
        drawing::draw_text_with_pool(self, pool, x, y, text, style);
    }

    /// Draw a single grapheme at position, allocating from pool if needed.
    pub fn draw_char_with_pool(
        &mut self,
        pool: &mut crate::grapheme_pool::GraphemePool,
        x: u32,
        y: u32,
        grapheme: &str,
        style: Style,
    ) {
        drawing::draw_char_with_pool(self, pool, x, y, grapheme, style);
    }

    /// Draw a box border.
    pub fn draw_box(&mut self, x: u32, y: u32, w: u32, h: u32, style: BoxStyle) {
        drawing::draw_box(self, x, y, w, h, style);
    }

    /// Draw a box border with extended options.
    pub fn draw_box_with_options(&mut self, x: u32, y: u32, w: u32, h: u32, options: BoxOptions) {
        drawing::draw_box_with_options(self, x, y, w, h, options);
    }

    /// Draw a text buffer view to this buffer.
    ///
    /// This is a convenience method that calls [`TextBufferView::render_to`].
    /// For rendering with grapheme pool support, use [`Self::draw_text_buffer_view_with_pool`].
    ///
    /// # Arguments
    /// * `view` - The text buffer view to render
    /// * `x`, `y` - Destination position in the buffer
    pub fn draw_text_buffer_view(&mut self, view: &TextBufferView<'_>, x: i32, y: i32) {
        view.render_to(self, x, y);
    }

    /// Draw a text buffer view to this buffer with grapheme pool support.
    ///
    /// This is a convenience method that calls [`TextBufferView::render_to_with_pool`].
    pub fn draw_text_buffer_view_with_pool(
        &mut self,
        view: &TextBufferView<'_>,
        pool: &mut GraphemePool,
        x: i32,
        y: i32,
    ) {
        view.render_to_with_pool(self, pool, x, y);
    }

    /// Draw an editor view to this buffer.
    ///
    /// This is a convenience method that calls [`EditorView::render_to`].
    ///
    /// # Arguments
    /// * `view` - The editor view to render
    /// * `x`, `y` - Destination position in the buffer
    /// * `width`, `height` - Dimensions of the rendering area
    pub fn draw_editor_view(
        &mut self,
        view: &mut EditorView,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        view.render_to(self, x, y, width, height);
    }

    // Scissor stack operations

    /// Push a scissor rectangle onto the stack.
    pub fn push_scissor(&mut self, rect: ClipRect) {
        self.scissor_stack.push(rect);
    }

    /// Pop the top scissor rectangle.
    pub fn pop_scissor(&mut self) {
        self.scissor_stack.pop();
    }

    /// Clear the scissor stack.
    pub fn clear_scissors(&mut self) {
        self.scissor_stack.clear();
    }

    // Opacity stack operations

    /// Push an opacity value onto the stack.
    pub fn push_opacity(&mut self, opacity: f32) {
        self.opacity_stack.push(opacity);
    }

    /// Pop the top opacity value.
    pub fn pop_opacity(&mut self) {
        self.opacity_stack.pop();
    }

    /// Get the current combined opacity.
    #[must_use]
    pub fn current_opacity(&self) -> f32 {
        self.opacity_stack.current()
    }

    /// Draw another buffer onto this one.
    pub fn draw_buffer(&mut self, x: i32, y: i32, src: &OptimizedBuffer) {
        self.draw_buffer_region(x, y, src, 0, 0, src.width, src.height, true);
    }

    /// Draw another buffer onto this one, updating grapheme pool counts.
    pub fn draw_buffer_with_pool(
        &mut self,
        pool: &mut GraphemePool,
        x: i32,
        y: i32,
        src: &OptimizedBuffer,
    ) {
        self.draw_buffer_region_with_pool(pool, x, y, src, 0, 0, src.width, src.height, true);
    }

    /// Draw a region of another buffer onto this one.
    #[allow(clippy::similar_names)] // dest_x_start/dest_y_start are standard coordinate names
    pub fn draw_buffer_region(
        &mut self,
        x: i32,
        y: i32,
        src: &OptimizedBuffer,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        respect_alpha: bool,
    ) {
        // Clamp source region to source buffer dimensions
        let copy_w = src_w.min(src.width.saturating_sub(src_x));
        let copy_h = src_h.min(src.height.saturating_sub(src_y));

        if copy_w == 0 || copy_h == 0 {
            return;
        }

        // Calculate destination intersection with this buffer
        let dest_x_start = x.max(0) as u32;
        let dest_y_start = y.max(0) as u32;
        let dest_x_end = (x.saturating_add(copy_w as i32))
            .max(0)
            .min(self.width as i32) as u32;
        let dest_y_end = (y.saturating_add(copy_h as i32))
            .max(0)
            .min(self.height as i32) as u32;

        if dest_x_start >= dest_x_end || dest_y_start >= dest_y_end {
            return;
        }

        let opacity = self.opacity_stack.current();
        let use_blend = respect_alpha && self.respect_alpha;

        for dest_y in dest_y_start..dest_y_end {
            let sy = src_y + (dest_y as i32 - y) as u32;
            // Use checked arithmetic to prevent overflow on large dimensions
            let Some(src_row) = (sy as usize).checked_mul(src.width as usize) else {
                continue;
            };
            let Some(dest_row) = (dest_y as usize).checked_mul(self.width as usize) else {
                continue;
            };

            for dest_x in dest_x_start..dest_x_end {
                // Check scissor clip
                if !self.scissor_stack.contains(dest_x as i32, dest_y as i32) {
                    continue;
                }

                let sx = src_x + (dest_x as i32 - x) as u32;
                let Some(src_idx) = src_row.checked_add(sx as usize) else {
                    continue;
                };
                let Some(dest_idx) = dest_row.checked_add(dest_x as usize) else {
                    continue;
                };
                // Bounds check for safety
                if src_idx >= src.cells.len() || dest_idx >= self.cells.len() {
                    continue;
                }
                let src_cell = &src.cells[src_idx];
                let dest_cell = &mut self.cells[dest_idx];

                if use_blend {
                    let mut blended = *src_cell;
                    if opacity < 1.0 {
                        blended.blend_with_opacity(opacity);
                    }
                    *dest_cell = blended.blend_over(dest_cell);
                } else if opacity < 1.0 {
                    let mut blended = *src_cell;
                    blended.blend_with_opacity(opacity);
                    *dest_cell = blended;
                } else {
                    *dest_cell = *src_cell;
                }
            }
        }
    }

    /// Draw a region of another buffer onto this one, updating grapheme pool counts.
    #[allow(clippy::similar_names)] // dest_x_start/dest_y_start are standard coordinate names
    pub fn draw_buffer_region_with_pool(
        &mut self,
        pool: &mut GraphemePool,
        x: i32,
        y: i32,
        src: &OptimizedBuffer,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        respect_alpha: bool,
    ) {
        // Clamp source region to source buffer dimensions
        let copy_w = src_w.min(src.width.saturating_sub(src_x));
        let copy_h = src_h.min(src.height.saturating_sub(src_y));

        if copy_w == 0 || copy_h == 0 {
            return;
        }

        // Calculate destination intersection with this buffer
        let dest_x_start = x.max(0) as u32;
        let dest_y_start = y.max(0) as u32;
        let dest_x_end = (x.saturating_add(copy_w as i32))
            .max(0)
            .min(self.width as i32) as u32;
        let dest_y_end = (y.saturating_add(copy_h as i32))
            .max(0)
            .min(self.height as i32) as u32;

        if dest_x_start >= dest_x_end || dest_y_start >= dest_y_end {
            return;
        }

        let opacity = self.opacity_stack.current();
        let use_blend = respect_alpha && self.respect_alpha;

        for dest_y in dest_y_start..dest_y_end {
            let sy = src_y + (dest_y as i32 - y) as u32;
            // Use checked arithmetic to prevent overflow on large dimensions
            let Some(src_row) = (sy as usize).checked_mul(src.width as usize) else {
                continue;
            };
            let Some(dest_row) = (dest_y as usize).checked_mul(self.width as usize) else {
                continue;
            };

            for dest_x in dest_x_start..dest_x_end {
                // Check scissor clip
                if !self.scissor_stack.contains(dest_x as i32, dest_y as i32) {
                    continue;
                }

                let sx = src_x + (dest_x as i32 - x) as u32;
                let Some(src_idx) = src_row.checked_add(sx as usize) else {
                    continue;
                };
                let Some(dest_idx) = dest_row.checked_add(dest_x as usize) else {
                    continue;
                };
                // Bounds check for safety
                if src_idx >= src.cells.len() || dest_idx >= self.cells.len() {
                    continue;
                }
                let src_cell = &src.cells[src_idx];
                let dest_cell = &mut self.cells[dest_idx];

                let old_content = dest_cell.content;
                let mut new_cell = *src_cell;
                if use_blend {
                    if opacity < 1.0 {
                        new_cell.blend_with_opacity(opacity);
                    }
                    new_cell = new_cell.blend_over(dest_cell);
                } else if opacity < 1.0 {
                    new_cell.blend_with_opacity(opacity);
                }

                let new_content = new_cell.content;
                let new_from_src = !use_blend || !src_cell.content.is_empty();

                if new_from_src {
                    if let CellContent::Grapheme(id) = new_content {
                        if id.pool_id() != 0 {
                            pool.incref(id);
                        }
                    }
                }

                if old_content != new_content {
                    if let CellContent::Grapheme(id) = old_content {
                        if id.pool_id() != 0 {
                            pool.decref(id);
                        }
                    }
                } else if new_from_src {
                    if let CellContent::Grapheme(id) = new_content {
                        if id.pool_id() != 0 {
                            pool.decref(id);
                        }
                    }
                }

                *dest_cell = new_cell;
            }
        }
    }

    /// Resize buffer, clearing contents.
    ///
    /// Uses saturating multiplication to prevent overflow for extremely large dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let size = (width as usize).saturating_mul(height as usize);
        self.cells = vec![Cell::clear(Rgba::TRANSPARENT); size];
        self.scissor_stack.clear();
        self.opacity_stack.clear();
        self.respect_alpha = true;
    }

    /// Release all grapheme references in this buffer.
    pub fn release_graphemes(&mut self, pool: &mut GraphemePool) {
        for cell in &self.cells {
            if let CellContent::Grapheme(id) = cell.content {
                if id.pool_id() != 0 {
                    pool.decref(id);
                }
            }
        }
    }

    /// Resize buffer, clearing contents and releasing grapheme references.
    pub fn resize_with_pool(&mut self, pool: &mut GraphemePool, width: u32, height: u32) {
        self.release_graphemes(pool);
        self.resize(width, height);
    }

    /// Enable or disable alpha blending for blended operations.
    pub fn set_respect_alpha(&mut self, enabled: bool) {
        self.respect_alpha = enabled;
    }

    /// Check whether alpha blending is enabled.
    #[must_use]
    pub fn respect_alpha(&self) -> bool {
        self.respect_alpha
    }

    /// Get raw cell slice.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Get mutable raw cell slice.
    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// Iterate over cells with positions.
    pub fn iter_cells(&self) -> impl Iterator<Item = (u32, u32, &Cell)> {
        self.cells.iter().enumerate().map(|(i, cell)| {
            let x = (i as u32) % self.width;
            let y = (i as u32) / self.width;
            (x, y, cell)
        })
    }
}

impl Default for OptimizedBuffer {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact float comparison is intentional in tests
    use super::*;

    // =========================================================================
    // Buffer Creation & Sizing
    // =========================================================================

    #[test]
    fn test_buffer_creation() {
        let buf = OptimizedBuffer::new(80, 24);
        assert_eq!(buf.width(), 80);
        assert_eq!(buf.height(), 24);
    }

    #[test]
    fn test_buffer_create_dimensions() {
        let buf = OptimizedBuffer::new(120, 40);
        assert_eq!(buf.size(), (120, 40));
        assert_eq!(buf.cells().len(), 120 * 40);
    }

    #[test]
    fn test_buffer_resize_larger() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.set(5, 5, Cell::new('X', Style::NONE));

        // Resize larger
        buf.resize(20, 20);

        assert_eq!(buf.width(), 20);
        assert_eq!(buf.height(), 20);
        // Contents are cleared on resize
        let cell = buf.get(5, 5).unwrap();
        assert!(cell.content.is_empty());
    }

    #[test]
    fn test_buffer_resize_smaller() {
        let mut buf = OptimizedBuffer::new(20, 20);
        buf.set(15, 15, Cell::new('X', Style::NONE));

        // Resize smaller
        buf.resize(10, 10);

        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 10);
        // Cell at (15, 15) is now out of bounds
        assert!(buf.get(15, 15).is_none());
    }

    #[test]
    fn test_buffer_clear() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::BLUE);

        for cell in buf.cells() {
            assert_eq!(cell.bg, Rgba::BLUE);
        }
    }

    #[test]
    fn test_buffer_default() {
        let buf = OptimizedBuffer::default();
        assert_eq!(buf.width(), 80);
        assert_eq!(buf.height(), 24);
    }

    #[test]
    fn test_buffer_with_id() {
        let buf = OptimizedBuffer::new(10, 10).with_id("main");
        assert_eq!(buf.id(), "main");
    }

    #[test]
    fn test_buffer_byte_size() {
        let buf = OptimizedBuffer::new(10, 10);
        let expected = 100 * std::mem::size_of::<Cell>();
        assert_eq!(buf.byte_size(), expected);
    }

    // =========================================================================
    // Cell Access
    // =========================================================================

    #[test]
    fn test_buffer_get_set() {
        let mut buf = OptimizedBuffer::new(10, 10);
        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(5, 5, cell);

        let retrieved = buf.get(5, 5).unwrap();
        assert_eq!(retrieved.fg, Rgba::RED);
    }

    #[test]
    fn test_buffer_get_set_cell() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Set various cells
        buf.set(0, 0, Cell::new('A', Style::NONE));
        buf.set(9, 9, Cell::new('Z', Style::NONE));
        buf.set(5, 5, Cell::new('M', Style::NONE));

        // Verify
        assert!(matches!(
            buf.get(0, 0).unwrap().content,
            CellContent::Char('A')
        ));
        assert!(matches!(
            buf.get(9, 9).unwrap().content,
            CellContent::Char('Z')
        ));
        assert!(matches!(
            buf.get(5, 5).unwrap().content,
            CellContent::Char('M')
        ));
    }

    #[test]
    fn test_buffer_bounds() {
        let buf = OptimizedBuffer::new(10, 10);
        assert!(buf.get(0, 0).is_some());
        assert!(buf.get(9, 9).is_some());
        assert!(buf.get(10, 10).is_none());
    }

    #[test]
    fn test_buffer_bounds_check() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Out-of-bounds set should be silently ignored
        buf.set(100, 100, Cell::new('X', Style::NONE));

        // Out-of-bounds get returns None
        assert!(buf.get(100, 100).is_none());
        assert!(buf.get(10, 0).is_none());
        assert!(buf.get(0, 10).is_none());
    }

    #[test]
    fn test_cell_index_overflow_protection() {
        // Test that cell_index() uses checked arithmetic to prevent overflow
        let buf = OptimizedBuffer::new(100, 100);

        // Normal coordinates work
        assert!(buf.get(50, 50).is_some());
        assert!(buf.get(0, 0).is_some());
        assert!(buf.get(99, 99).is_some());

        // Out of bounds returns None
        assert!(buf.get(100, 0).is_none());
        assert!(buf.get(0, 100).is_none());

        // Very large coordinates that would overflow u32 multiplication
        // should return None instead of wrapping around
        assert!(buf.get(u32::MAX, 0).is_none());
        assert!(buf.get(0, u32::MAX).is_none());
        assert!(buf.get(u32::MAX, u32::MAX).is_none());

        // Large y values are rejected by bounds check before arithmetic
        // This tests that even values close to u32::MAX are handled safely
        let large_y = u32::MAX - 1;
        assert!(buf.get(0, large_y).is_none());
    }

    #[test]
    fn test_buffer_get_mut() {
        let mut buf = OptimizedBuffer::new(10, 10);

        if let Some(cell) = buf.get_mut(5, 5) {
            cell.fg = Rgba::GREEN;
        }

        assert_eq!(buf.get(5, 5).unwrap().fg, Rgba::GREEN);
    }

    #[test]
    fn test_buffer_fill_rect() {
        let mut buf = OptimizedBuffer::new(20, 20);
        buf.fill_rect(5, 5, 10, 10, Rgba::RED);

        // Inside filled region
        assert_eq!(buf.get(5, 5).unwrap().bg, Rgba::RED);
        assert_eq!(buf.get(14, 14).unwrap().bg, Rgba::RED);
        assert_eq!(buf.get(10, 10).unwrap().bg, Rgba::RED);

        // Outside filled region
        assert_eq!(buf.get(0, 0).unwrap().bg, Rgba::TRANSPARENT);
        assert_eq!(buf.get(4, 4).unwrap().bg, Rgba::TRANSPARENT);
        assert_eq!(buf.get(15, 15).unwrap().bg, Rgba::TRANSPARENT);
    }

    #[test]
    fn test_buffer_fill_rect_edge_cases() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Zero width/height should do nothing
        buf.fill_rect(5, 5, 0, 5, Rgba::RED);
        buf.fill_rect(5, 5, 5, 0, Rgba::RED);
        assert_eq!(buf.get(5, 5).unwrap().bg, Rgba::TRANSPARENT);

        // Fill extends past buffer edge (should be clipped)
        buf.fill_rect(8, 8, 10, 10, Rgba::BLUE);
        assert_eq!(buf.get(8, 8).unwrap().bg, Rgba::BLUE);
        assert_eq!(buf.get(9, 9).unwrap().bg, Rgba::BLUE);
    }

    // =========================================================================
    // Scissor Stack
    // =========================================================================

    #[test]
    fn test_scissor_push_pop() {
        let mut buf = OptimizedBuffer::new(20, 20);

        // Initially all visible
        buf.set(0, 0, Cell::new('A', Style::NONE));
        buf.set(19, 19, Cell::new('B', Style::NONE));
        assert!(matches!(
            buf.get(0, 0).unwrap().content,
            CellContent::Char('A')
        ));
        assert!(matches!(
            buf.get(19, 19).unwrap().content,
            CellContent::Char('B')
        ));

        // Push scissor to restrict to center region
        buf.push_scissor(ClipRect::new(5, 5, 10, 10));

        // Set inside scissor should work
        buf.set(10, 10, Cell::new('C', Style::NONE));
        assert!(matches!(
            buf.get(10, 10).unwrap().content,
            CellContent::Char('C')
        ));

        // Set outside scissor should be ignored
        buf.set(0, 0, Cell::new('X', Style::NONE));
        // Should still be 'A' from before
        assert!(matches!(
            buf.get(0, 0).unwrap().content,
            CellContent::Char('A')
        ));

        // Pop scissor
        buf.pop_scissor();

        // Now (0, 0) should be writable again
        buf.set(0, 0, Cell::new('Y', Style::NONE));
        assert!(matches!(
            buf.get(0, 0).unwrap().content,
            CellContent::Char('Y')
        ));
    }

    #[test]
    fn test_scissor_intersection() {
        let mut buf = OptimizedBuffer::new(30, 30);

        // Push outer scissor (5, 5) to (25, 25)
        buf.push_scissor(ClipRect::new(5, 5, 20, 20));

        // Push inner scissor (10, 10) to (20, 20)
        buf.push_scissor(ClipRect::new(10, 10, 10, 10));

        // Set inside inner scissor should work
        buf.set(15, 15, Cell::new('I', Style::NONE));
        assert!(matches!(
            buf.get(15, 15).unwrap().content,
            CellContent::Char('I')
        ));

        // Set outside inner but inside outer should be ignored
        buf.set(7, 7, Cell::new('O', Style::NONE));
        // Should remain empty
        assert!(buf.get(7, 7).unwrap().content.is_empty());

        // Pop inner scissor
        buf.pop_scissor();

        // Now (7, 7) should be writable
        buf.set(7, 7, Cell::new('O', Style::NONE));
        assert!(matches!(
            buf.get(7, 7).unwrap().content,
            CellContent::Char('O')
        ));
    }

    #[test]
    fn test_scissor_outside_bounds() {
        let mut buf = OptimizedBuffer::new(20, 20);

        // Push scissor that's completely outside buffer bounds
        buf.push_scissor(ClipRect::new(100, 100, 10, 10));

        // Any set should be ignored
        buf.set(5, 5, Cell::new('X', Style::NONE));
        assert!(buf.get(5, 5).unwrap().content.is_empty());

        buf.pop_scissor();

        // Now set should work
        buf.set(5, 5, Cell::new('Y', Style::NONE));
        assert!(matches!(
            buf.get(5, 5).unwrap().content,
            CellContent::Char('Y')
        ));
    }

    #[test]
    fn test_scissor_fill_rect_interaction() {
        let mut buf = OptimizedBuffer::new(20, 20);

        // Restrict to center region
        buf.push_scissor(ClipRect::new(5, 5, 10, 10));

        // Fill entire buffer - should only fill scissor region
        buf.fill_rect(0, 0, 20, 20, Rgba::RED);

        // Inside scissor should be filled
        assert_eq!(buf.get(10, 10).unwrap().bg, Rgba::RED);

        // Outside scissor should NOT be filled
        assert_eq!(buf.get(0, 0).unwrap().bg, Rgba::TRANSPARENT);
        assert_eq!(buf.get(19, 19).unwrap().bg, Rgba::TRANSPARENT);
    }

    #[test]
    fn test_scissor_clear_scissors() {
        let mut buf = OptimizedBuffer::new(20, 20);

        buf.push_scissor(ClipRect::new(5, 5, 10, 10));
        buf.push_scissor(ClipRect::new(7, 7, 5, 5));

        // Clear all scissors
        buf.clear_scissors();

        // Now entire buffer should be visible
        buf.set(0, 0, Cell::new('X', Style::NONE));
        assert!(matches!(
            buf.get(0, 0).unwrap().content,
            CellContent::Char('X')
        ));
    }

    // =========================================================================
    // Opacity Stack
    // =========================================================================

    #[test]
    fn test_opacity_push_pop() {
        let mut buf = OptimizedBuffer::new(10, 10);

        assert_eq!(buf.current_opacity(), 1.0);

        buf.push_opacity(0.5);
        assert!((buf.current_opacity() - 0.5).abs() < 0.01);

        buf.push_opacity(0.5);
        // Should multiply: 0.5 * 0.5 = 0.25
        assert!((buf.current_opacity() - 0.25).abs() < 0.01);

        buf.pop_opacity();
        assert!((buf.current_opacity() - 0.5).abs() < 0.01);

        buf.pop_opacity();
        assert_eq!(buf.current_opacity(), 1.0);
    }

    #[test]
    fn test_opacity_blending() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Set without opacity
        buf.set(0, 0, Cell::new('A', Style::fg(Rgba::RED)));
        let cell_no_opacity = *buf.get(0, 0).unwrap();
        assert_eq!(cell_no_opacity.fg.a, 1.0);

        // Set with 50% opacity
        buf.push_opacity(0.5);
        buf.set(1, 0, Cell::new('B', Style::fg(Rgba::GREEN)));
        let cell_with_opacity = *buf.get(1, 0).unwrap();
        // Alpha should be reduced
        assert!(cell_with_opacity.fg.a < 1.0);
        assert!((cell_with_opacity.fg.a - 0.5).abs() < 0.01);

        buf.pop_opacity();
    }

    #[test]
    fn test_opacity_affects_fill_rect() {
        let mut buf = OptimizedBuffer::new(10, 10);

        buf.push_opacity(0.5);
        buf.fill_rect(0, 0, 5, 5, Rgba::RED);

        let cell = buf.get(2, 2).unwrap();
        // Alpha should be reduced
        assert!(cell.bg.a < 1.0);
    }

    // =========================================================================
    // Buffer Drawing / Compositing
    // =========================================================================

    #[test]
    fn test_draw_buffer_region() {
        let mut src = OptimizedBuffer::new(4, 4);
        src.set(1, 1, Cell::new('X', Style::fg(Rgba::RED)));

        let mut dst = OptimizedBuffer::new(4, 4);
        dst.draw_buffer_region(0, 0, &src, 1, 1, 1, 1, true);

        assert_eq!(
            dst.get(0, 0).unwrap().content,
            crate::cell::CellContent::Char('X')
        );
    }

    #[test]
    fn test_draw_buffer_negative_out_of_bounds() {
        let mut src = OptimizedBuffer::new(10, 10);
        src.fill_rect(0, 0, 10, 10, Rgba::RED);

        let mut dst = OptimizedBuffer::new(10, 10);
        // Draw src at -20, -20. Width is 10.
        // End x is -10.
        // Should draw nothing and definitely not crash/hang.
        dst.draw_buffer_region(-20, -20, &src, 0, 0, 10, 10, true);

        // Verify dst is still empty/transparent
        let cell = dst.get(0, 0).unwrap();
        assert_eq!(cell.bg, Rgba::TRANSPARENT);
    }

    #[test]
    fn test_draw_buffer() {
        let mut src = OptimizedBuffer::new(5, 5);
        src.fill_rect(0, 0, 5, 5, Rgba::BLUE);

        let mut dst = OptimizedBuffer::new(10, 10);
        dst.draw_buffer(2, 2, &src);

        // Check copied region
        assert_eq!(dst.get(2, 2).unwrap().bg, Rgba::BLUE);
        assert_eq!(dst.get(6, 6).unwrap().bg, Rgba::BLUE);

        // Check outside region
        assert_eq!(dst.get(0, 0).unwrap().bg, Rgba::TRANSPARENT);
    }

    // =========================================================================
    // Alpha Blending
    // =========================================================================

    #[test]
    fn test_set_blended() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // First, set a background
        buf.set(0, 0, Cell::clear(Rgba::RED));

        // Then blend a semi-transparent cell over it
        let overlay = Cell::clear(Rgba::new(0.0, 1.0, 0.0, 0.5)); // 50% green
        buf.set_blended(0, 0, overlay);

        let result = buf.get(0, 0).unwrap();
        // Result should be a blend of red and green
        assert!(result.bg.g > 0.0); // Has some green
        assert!(result.bg.r > 0.0); // Has some red (from background)
    }

    #[test]
    fn test_respect_alpha_flag() {
        let mut buf = OptimizedBuffer::new(10, 10);
        assert!(buf.respect_alpha());

        buf.set_respect_alpha(false);
        assert!(!buf.respect_alpha());

        // With respect_alpha disabled, blended operations should replace
        buf.set(0, 0, Cell::clear(Rgba::RED));
        let overlay = Cell::clear(Rgba::new(0.0, 1.0, 0.0, 0.5));
        buf.set_blended(0, 0, overlay);

        // Should be the overlay color directly, not blended
        let result = buf.get(0, 0).unwrap();
        assert_eq!(result.bg.g, 1.0);
        assert_eq!(result.bg.r, 0.0);
    }

    // =========================================================================
    // Iterator
    // =========================================================================

    #[test]
    fn test_iter_cells() {
        let buf = OptimizedBuffer::new(3, 3);
        let mut count = 0;

        for (x, y, _cell) in buf.iter_cells() {
            assert!(x < 3);
            assert!(y < 3);
            count += 1;
        }

        assert_eq!(count, 9);
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_zero_size_buffer() {
        // Zero dimensions are clamped to 1 to prevent division by zero in iter_cells
        let buf = OptimizedBuffer::new(0, 0);
        assert_eq!(buf.width(), 1);
        assert_eq!(buf.height(), 1);
        assert_eq!(buf.cells().len(), 1);
    }

    #[test]
    fn test_large_buffer() {
        // Test large buffer doesn't overflow
        let buf = OptimizedBuffer::new(1000, 1000);
        assert_eq!(buf.cells().len(), 1_000_000);
    }
}
