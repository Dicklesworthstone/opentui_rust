//! Cell-based frame buffer with alpha blending and scissoring.

// Buffer operations naturally have many parameters for region copying
#![allow(clippy::too_many_arguments)]

mod drawing;
mod opacity;
mod scissor;

pub use drawing::{BoxOptions, BoxSides, BoxStyle, TitleAlign};
pub use opacity::OpacityStack;
pub use scissor::{ClipRect, ScissorStack};

use crate::cell::Cell;
use crate::color::Rgba;
use crate::style::Style;

/// Optimized cell buffer for terminal rendering.
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

    /// Check if position is within current scissor rect.
    fn is_visible(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.scissor_stack.contains(x as i32, y as i32)
    }

    /// Clear entire buffer with background color.
    pub fn clear(&mut self, bg: Rgba) {
        for cell in &mut self.cells {
            *cell = Cell::clear(bg);
        }
    }

    /// Fill a rectangular region with background color.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, bg: Rgba) {
        for row in y..y.saturating_add(h).min(self.height) {
            for col in x..x.saturating_add(w).min(self.width) {
                self.set(col, row, Cell::clear(bg));
            }
        }
    }

    /// Draw text at position with style.
    pub fn draw_text(&mut self, x: u32, y: u32, text: &str, style: Style) {
        drawing::draw_text(self, x, y, text, style);
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
        let max_y = (src_y + src_h).min(src.height);
        let max_x = (src_x + src_w).min(src.width);

        for sy in src_y..max_y {
            let dest_y = y + (sy - src_y) as i32;
            if dest_y < 0 || dest_y >= self.height as i32 {
                continue;
            }

            for sx in src_x..max_x {
                let dest_x = x + (sx - src_x) as i32;
                if dest_x < 0 || dest_x >= self.width as i32 {
                    continue;
                }

                if let Some(cell) = src.get(sx, sy) {
                    if respect_alpha {
                        self.set_blended(dest_x as u32, dest_y as u32, cell.clone());
                    } else {
                        self.set(dest_x as u32, dest_y as u32, cell.clone());
                    }
                }
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
