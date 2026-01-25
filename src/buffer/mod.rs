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
mod scissor;

pub use drawing::{BoxOptions, BoxSides, BoxStyle, TitleAlign};
pub use opacity::OpacityStack;
pub use scissor::{ClipRect, ScissorStack};

use crate::cell::{Cell, CellContent};
use crate::color::Rgba;
use crate::grapheme_pool::GraphemePool;
use crate::style::Style;

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
}

impl OptimizedBuffer {
    /// Create a new buffer with the given dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            cells: vec![Cell::clear(Rgba::TRANSPARENT); size],
            scissor_stack: ScissorStack::new(),
            opacity_stack: OpacityStack::new(),
            id: String::new(),
            respect_alpha: true,
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

    /// Get cell at position.
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> Option<&Cell> {
        if x < self.width && y < self.height {
            Some(&self.cells[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    /// Get mutable cell at position.
    pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            Some(&mut self.cells[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    /// Set cell at position, respecting scissor and opacity.
    pub fn set(&mut self, x: u32, y: u32, mut cell: Cell) {
        if !self.is_visible(x, y) {
            return;
        }

        let opacity = self.opacity_stack.current();
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        if let Some(dest) = self.get_mut(x, y) {
            *dest = cell;
        }
    }

    /// Set cell at position, updating grapheme pool reference counts.
    pub fn set_with_pool(&mut self, pool: &mut GraphemePool, x: u32, y: u32, mut cell: Cell) {
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
    pub fn set_blended(&mut self, x: u32, y: u32, mut cell: Cell) {
        if !self.is_visible(x, y) {
            return;
        }

        let opacity = self.opacity_stack.current();
        if opacity < 1.0 {
            cell.blend_with_opacity(opacity);
        }

        let respect_alpha = self.respect_alpha;
        if let Some(dest) = self.get_mut(x, y) {
            if respect_alpha {
                *dest = cell.blend_over(dest);
            } else {
                *dest = cell;
            }
        }
    }

    /// Set cell with alpha blending over existing content, updating grapheme pool counts.
    pub fn set_blended_with_pool(
        &mut self,
        pool: &mut GraphemePool,
        x: u32,
        y: u32,
        mut cell: Cell,
    ) {
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

    /// Clear entire buffer with background color.
    pub fn clear(&mut self, bg: Rgba) {
        // Create the clear cell once and fill the entire buffer
        // This is more efficient than creating Cell::clear(bg) per cell
        let clear_cell = Cell::clear(bg);
        self.cells.fill(clear_cell);
    }

    /// Clear entire buffer with background color, updating grapheme pool counts.
    pub fn clear_with_pool(&mut self, pool: &mut GraphemePool, bg: Rgba) {
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
        let dest_x_end = (x.saturating_add(copy_w as i32)).min(self.width as i32) as u32;
        let dest_y_end = (y.saturating_add(copy_h as i32)).min(self.height as i32) as u32;

        if dest_x_start >= dest_x_end || dest_y_start >= dest_y_end {
            return;
        }

        let opacity = self.opacity_stack.current();
        let use_blend = respect_alpha && self.respect_alpha;

        for dest_y in dest_y_start..dest_y_end {
            let sy = src_y + (dest_y as i32 - y) as u32;
            let src_row = (sy * src.width) as usize;
            let dest_row = (dest_y * self.width) as usize;

            for dest_x in dest_x_start..dest_x_end {
                // Check scissor clip
                if !self.scissor_stack.contains(dest_x as i32, dest_y as i32) {
                    continue;
                }

                let sx = src_x + (dest_x as i32 - x) as u32;
                let src_idx = src_row + sx as usize;
                let dest_idx = dest_row + dest_x as usize;
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
        let dest_x_end = (x.saturating_add(copy_w as i32)).min(self.width as i32) as u32;
        let dest_y_end = (y.saturating_add(copy_h as i32)).min(self.height as i32) as u32;

        if dest_x_start >= dest_x_end || dest_y_start >= dest_y_end {
            return;
        }

        let opacity = self.opacity_stack.current();
        let use_blend = respect_alpha && self.respect_alpha;

        for dest_y in dest_y_start..dest_y_end {
            let sy = src_y + (dest_y as i32 - y) as u32;
            let src_row = (sy * src.width) as usize;
            let dest_row = (dest_y * self.width) as usize;

            for dest_x in dest_x_start..dest_x_end {
                // Check scissor clip
                if !self.scissor_stack.contains(dest_x as i32, dest_y as i32) {
                    continue;
                }

                let sx = src_x + (dest_x as i32 - x) as u32;
                let src_idx = src_row + sx as usize;
                let dest_idx = dest_row + dest_x as usize;
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
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.cells = vec![Cell::clear(Rgba::TRANSPARENT); (width * height) as usize];
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
    use super::*;

    #[test]
    fn test_buffer_creation() {
        let buf = OptimizedBuffer::new(80, 24);
        assert_eq!(buf.width(), 80);
        assert_eq!(buf.height(), 24);
    }

    #[test]
    fn test_buffer_get_set() {
        let mut buf = OptimizedBuffer::new(10, 10);
        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(5, 5, cell);

        let retrieved = buf.get(5, 5).unwrap();
        assert_eq!(retrieved.fg, Rgba::RED);
    }

    #[test]
    fn test_buffer_bounds() {
        let buf = OptimizedBuffer::new(10, 10);
        assert!(buf.get(0, 0).is_some());
        assert!(buf.get(9, 9).is_some());
        assert!(buf.get(10, 10).is_none());
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
}
