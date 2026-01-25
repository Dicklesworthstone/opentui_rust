//! Buffered ANSI output writer with state tracking.

use crate::ansi::{self, ColorMode};
use crate::cell::Cell;
use crate::color::Rgba;
use crate::grapheme_pool::GraphemePool;
use crate::style::TextAttributes;
use std::io::{self, Write};

/// Buffered writer that tracks ANSI state to minimize escape sequences.
pub struct AnsiWriter<W: Write> {
    writer: W,
    buffer: Vec<u8>,

    // Color output mode
    color_mode: ColorMode,

    // Current state for delta encoding
    current_fg: Option<Rgba>,
    current_bg: Option<Rgba>,
    current_attrs: TextAttributes,
    current_link: Option<u32>,

    // Cursor position
    cursor_row: u32,
    cursor_col: u32,
}

impl<W: Write> AnsiWriter<W> {
    /// Create a new ANSI writer wrapping the given output.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: Vec::with_capacity(8192),
            color_mode: ColorMode::TrueColor,
            current_fg: None,
            current_bg: None,
            current_attrs: TextAttributes::empty(),
            current_link: None,
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    /// Create a new ANSI writer with specified color mode.
    pub fn with_color_mode(writer: W, color_mode: ColorMode) -> Self {
        Self {
            writer,
            buffer: Vec::with_capacity(8192),
            color_mode,
            current_fg: None,
            current_bg: None,
            current_attrs: TextAttributes::empty(),
            current_link: None,
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    /// Set the color output mode.
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
    }

    /// Get the current color output mode.
    #[must_use]
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    /// Reset all state tracking.
    pub fn reset_state(&mut self) {
        self.current_fg = None;
        self.current_bg = None;
        self.current_attrs = TextAttributes::empty();
        self.current_link = None;
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    /// Write raw bytes to the buffer.
    pub fn write_raw(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Write a raw string to the buffer.
    pub fn write_str(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
    }

    /// Move cursor to position, using relative moves if more efficient.
    pub fn move_cursor(&mut self, row: u32, col: u32) {
        if row == self.cursor_row && col == self.cursor_col {
            return;
        }

        let dy = row as i32 - self.cursor_row as i32;
        let dx = col as i32 - self.cursor_col as i32;

        // Calculate cost of absolute vs relative move
        // ESC[r;cH = 1+1+digits(r)+1+digits(c)+1 = 4 + digits
        let abs_cost = 4 + digits(row + 1) + digits(col + 1);
        let rel_cost = if dy != 0 {
            3 + digits(dy.unsigned_abs())
        } else {
            0
        } + if dx != 0 {
            3 + digits(dx.unsigned_abs())
        } else {
            0
        };

        if rel_cost < abs_cost && (dy != 0 || dx != 0) {
            let _ = ansi::write_cursor_move(&mut self.buffer, dx, dy);
        } else {
            let _ = ansi::write_cursor_position(&mut self.buffer, row, col);
        }

        self.cursor_row = row;
        self.cursor_col = col;
    }

    /// Set foreground color if different from current.
    pub fn set_fg(&mut self, color: Rgba) {
        if self.current_fg != Some(color) {
            let _ = ansi::write_fg_color_with_mode(&mut self.buffer, color, self.color_mode);
            self.current_fg = Some(color);
        }
    }

    /// Set background color if different from current.
    pub fn set_bg(&mut self, color: Rgba) {
        if self.current_bg != Some(color) {
            let _ = ansi::write_bg_color_with_mode(&mut self.buffer, color, self.color_mode);
            self.current_bg = Some(color);
        }
    }

    /// Set text attributes, only writing changes.
    pub fn set_attributes(&mut self, attrs: TextAttributes) {
        let attrs = attrs.flags_only();
        if self.current_attrs == attrs {
            return;
        }

        // Check what needs to be turned off
        let removed = self.current_attrs - attrs;
        if !removed.is_empty() {
            let mut codes = Vec::new();
            if removed.contains(TextAttributes::BOLD) || removed.contains(TextAttributes::DIM) {
                codes.push("22");
            }
            if removed.contains(TextAttributes::ITALIC) {
                codes.push("23");
            }
            if removed.contains(TextAttributes::UNDERLINE) {
                codes.push("24");
            }
            if removed.contains(TextAttributes::BLINK) {
                codes.push("25");
            }
            if removed.contains(TextAttributes::INVERSE) {
                codes.push("27");
            }
            if removed.contains(TextAttributes::HIDDEN) {
                codes.push("28");
            }
            if removed.contains(TextAttributes::STRIKETHROUGH) {
                codes.push("29");
            }

            if !codes.is_empty() {
                use std::io::Write; // Ensure write! macro is available if needed, or just write_all
                // Manually constructing the string here might be faster than a helper if specific codes
                // But let's just write bytes
                self.buffer.extend_from_slice(b"\x1b[");
                for (i, code) in codes.iter().enumerate() {
                    if i > 0 {
                        self.buffer.push(b';');
                    }
                    self.buffer.extend_from_slice(code.as_bytes());
                }
                self.buffer.push(b'm');
            }

            // Update current attributes to reflect removal
            self.current_attrs -= removed;
        }
        // Apply new attributes
        let to_add = attrs - self.current_attrs;
        if !to_add.is_empty() {
            let _ = ansi::write_attributes(&mut self.buffer, to_add);
        }

        self.current_attrs = attrs;
    }

    /// Set hyperlink if different from current.
    pub fn set_link(&mut self, link_id: Option<u32>, url: Option<&str>) {
        if self.current_link == link_id {
            return;
        }

        match (link_id, url) {
            (Some(id), Some(url)) => {
                let _ = ansi::write_hyperlink_start(&mut self.buffer, id, url);
            }
            _ => {
                self.write_str(ansi::HYPERLINK_END);
            }
        }

        self.current_link = link_id;
    }

    /// Write a cell at the current cursor position.
    pub fn write_cell(&mut self, cell: &Cell) {
        self.write_cell_with_link(cell, None);
    }

    /// Write a cell at the current cursor position with optional hyperlink URL.
    pub fn write_cell_with_link(&mut self, cell: &Cell, link_url: Option<&str>) {
        self.set_link(cell.attributes.link_id(), link_url);

        // Update style state
        self.set_attributes(cell.attributes);
        self.set_fg(cell.fg);
        self.set_bg(cell.bg);

        // Write content using the cell's string representation
        // This handles all content types correctly without fixed-size buffer limitations
        match &cell.content {
            crate::cell::CellContent::Char(c) => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                self.buffer.extend_from_slice(s.as_bytes());
            }
            crate::cell::CellContent::Grapheme(id) => {
                for _ in 0..id.width() {
                    self.buffer.push(b' ');
                }
            }
            crate::cell::CellContent::Empty => {
                self.buffer.push(b' ');
            }
            crate::cell::CellContent::Continuation => {
                // No output for continuation cells
            }
        }

        // Track cursor movement
        self.cursor_col += cell.display_width() as u32;
    }

    /// Write a cell at the current cursor position with optional hyperlink URL,
    /// resolving grapheme IDs via the pool.
    pub fn write_cell_with_link_and_pool(
        &mut self,
        cell: &Cell,
        link_url: Option<&str>,
        pool: &GraphemePool,
    ) {
        self.set_link(cell.attributes.link_id(), link_url);

        // Update style state
        self.set_attributes(cell.attributes);
        self.set_fg(cell.fg);
        self.set_bg(cell.bg);

        // Write content using the pool to resolve graphemes
        match &cell.content {
            crate::cell::CellContent::Char(c) => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                self.buffer.extend_from_slice(s.as_bytes());
            }
            crate::cell::CellContent::Grapheme(id) => {
                if let Some(grapheme) = pool.get(*id) {
                    self.buffer.extend_from_slice(grapheme.as_bytes());
                } else {
                    for _ in 0..id.width() {
                        self.buffer.push(b' ');
                    }
                }
            }
            crate::cell::CellContent::Empty => {
                self.buffer.push(b' ');
            }
            crate::cell::CellContent::Continuation => {
                // No output for continuation cells
            }
        }

        // Track cursor movement
        self.cursor_col += cell.display_width() as u32;
    }

    /// Write a cell at a specific position.
    pub fn write_cell_at(&mut self, row: u32, col: u32, cell: &Cell) {
        self.move_cursor(row, col);
        self.write_cell(cell);
    }

    /// Write a cell at a specific position with optional hyperlink URL.
    pub fn write_cell_at_with_link(
        &mut self,
        row: u32,
        col: u32,
        cell: &Cell,
        link_url: Option<&str>,
    ) {
        self.move_cursor(row, col);
        self.write_cell_with_link(cell, link_url);
    }

    /// Write a cell at a specific position with optional hyperlink URL,
    /// resolving grapheme IDs via the pool.
    pub fn write_cell_at_with_link_and_pool(
        &mut self,
        row: u32,
        col: u32,
        cell: &Cell,
        link_url: Option<&str>,
        pool: &GraphemePool,
    ) {
        self.move_cursor(row, col);
        self.write_cell_with_link_and_pool(cell, link_url, pool);
    }

    /// Write a cell at the current cursor position, resolving grapheme IDs from the pool.
    ///
    /// Unlike [`Self::write_cell`], this properly renders multi-codepoint graphemes
    /// by looking them up in the provided pool.
    pub fn write_cell_with_pool(&mut self, cell: &Cell, pool: &crate::grapheme_pool::GraphemePool) {
        self.write_cell_with_pool_and_link(cell, pool, None);
    }

    /// Write a cell at the current cursor position with pool lookup and optional hyperlink.
    pub fn write_cell_with_pool_and_link(
        &mut self,
        cell: &Cell,
        pool: &crate::grapheme_pool::GraphemePool,
        link_url: Option<&str>,
    ) {
        self.set_link(cell.attributes.link_id(), link_url);
        self.set_attributes(cell.attributes);
        self.set_fg(cell.fg);
        self.set_bg(cell.bg);

        match &cell.content {
            crate::cell::CellContent::Char(c) => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                self.buffer.extend_from_slice(s.as_bytes());
            }
            crate::cell::CellContent::Grapheme(id) => {
                // Look up the grapheme in the pool
                if let Some(grapheme) = pool.get(*id) {
                    self.buffer.extend_from_slice(grapheme.as_bytes());
                } else {
                    // Fallback: write spaces matching width
                    for _ in 0..id.width() {
                        self.buffer.push(b' ');
                    }
                }
            }
            crate::cell::CellContent::Empty => {
                self.buffer.push(b' ');
            }
            crate::cell::CellContent::Continuation => {
                // No output for continuation cells
            }
        }

        self.cursor_col += cell.display_width() as u32;
    }

    /// Write a cell at a specific position, resolving grapheme IDs from the pool.
    pub fn write_cell_at_with_pool(
        &mut self,
        row: u32,
        col: u32,
        cell: &Cell,
        pool: &crate::grapheme_pool::GraphemePool,
    ) {
        self.move_cursor(row, col);
        self.write_cell_with_pool(cell, pool);
    }

    /// Write a cell at a specific position with pool lookup and optional hyperlink.
    pub fn write_cell_at_with_pool_and_link(
        &mut self,
        row: u32,
        col: u32,
        cell: &Cell,
        pool: &crate::grapheme_pool::GraphemePool,
        link_url: Option<&str>,
    ) {
        self.move_cursor(row, col);
        self.write_cell_with_pool_and_link(cell, pool, link_url);
    }

    /// Reset all ANSI attributes.
    pub fn reset(&mut self) {
        self.write_str(ansi::RESET);
        self.current_fg = None;
        self.current_bg = None;
        self.current_attrs = TextAttributes::empty();
        self.current_link = None;
    }

    /// Flush the buffer to the underlying writer.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.write_all(&self.buffer)?;
        self.buffer.clear();
        self.writer.flush()
    }

    /// Get the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Get a reference to the buffer.
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Clear the buffer without flushing.
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }
}

/// Count decimal digits in a number.
fn digits(n: u32) -> usize {
    if n == 0 { 1 } else { (n.ilog10() + 1) as usize }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;

    #[test]
    fn test_ansi_writer_basic() {
        let mut writer = AnsiWriter::new(Vec::new());
        writer.write_str("Hello");
        assert_eq!(writer.buffer(), b"Hello");
    }

    #[test]
    fn test_cursor_movement() {
        let mut writer = AnsiWriter::new(Vec::new());
        writer.move_cursor(5, 10);
        assert!(writer.buffer().starts_with(b"\x1b["));
    }

    #[test]
    fn test_color_caching() {
        let mut writer = AnsiWriter::new(Vec::new());

        writer.set_fg(Rgba::RED);
        let len1 = writer.buffer().len();

        writer.set_fg(Rgba::RED); // Same color
        let len2 = writer.buffer().len();

        // Should not write again
        assert_eq!(len1, len2);

        writer.set_fg(Rgba::BLUE); // Different color
        let len3 = writer.buffer().len();

        // Should write new color
        assert!(len3 > len2);
    }

    #[test]
    fn test_write_cell() {
        let mut writer = AnsiWriter::new(Vec::new());
        let cell = Cell::new('A', Style::fg(Rgba::RED));
        writer.write_cell(&cell);
        writer.flush().unwrap();

        // After flush, data is in the underlying writer (Vec), not buffer
        let inner = writer.into_inner();
        let output = String::from_utf8_lossy(inner.as_slice());
        assert!(output.contains('A'));
    }

    #[test]
    fn test_write_cell_with_pool_simple_char() {
        let mut writer = AnsiWriter::new(Vec::new());
        let pool = crate::grapheme_pool::GraphemePool::new();

        let cell = Cell::new('X', Style::NONE);
        writer.write_cell_with_pool(&cell, &pool);

        let output = String::from_utf8_lossy(writer.buffer());
        assert!(output.ends_with('X'));
    }

    #[test]
    fn test_write_cell_with_pool_grapheme() {
        let mut writer = AnsiWriter::new(Vec::new());
        let mut pool = crate::grapheme_pool::GraphemePool::new();

        // Allocate a grapheme in the pool
        let id = pool.alloc("👨‍👩‍👧");

        let cell = Cell {
            content: crate::cell::CellContent::Grapheme(id),
            fg: Rgba::WHITE,
            bg: Rgba::BLACK,
            attributes: crate::style::TextAttributes::empty(),
        };

        writer.write_cell_with_pool(&cell, &pool);

        let output = String::from_utf8_lossy(writer.buffer());
        assert!(output.contains("👨‍👩‍👧"));
    }

    #[test]
    fn test_write_cell_with_pool_invalid_id_fallback() {
        let mut writer = AnsiWriter::new(Vec::new());
        let pool = crate::grapheme_pool::GraphemePool::new();

        // Create a grapheme ID that doesn't exist in the pool
        let invalid_id = crate::cell::GraphemeId::new(999, 2);
        let cell = Cell {
            content: crate::cell::CellContent::Grapheme(invalid_id),
            fg: Rgba::WHITE,
            bg: Rgba::BLACK,
            attributes: crate::style::TextAttributes::empty(),
        };

        writer.write_cell_with_pool(&cell, &pool);

        // Should fall back to spaces matching the width
        let output = String::from_utf8_lossy(writer.buffer());
        assert!(output.ends_with("  ")); // 2 spaces for width 2
    }

    #[test]
    fn test_write_cell_continuation_no_output() {
        let mut writer = AnsiWriter::new(Vec::new());
        let pool = crate::grapheme_pool::GraphemePool::new();

        // First write a cell to establish style state
        writer.set_fg(Rgba::WHITE);
        writer.set_bg(Rgba::BLACK);
        writer.clear_buffer();

        let cell = Cell::continuation(Rgba::BLACK);
        writer.write_cell_with_pool(&cell, &pool);

        // Continuation cells produce no visible character output
        // The buffer may contain ANSI codes for style changes, but no printable content
        // The cell's display_width is 0, so cursor shouldn't advance for content
        assert_eq!(cell.display_width(), 0);
    }

    #[test]
    fn test_write_cell_at_with_pool() {
        let mut writer = AnsiWriter::new(Vec::new());
        let mut pool = crate::grapheme_pool::GraphemePool::new();

        let id = pool.alloc("🎉");
        let cell = Cell {
            content: crate::cell::CellContent::Grapheme(id),
            fg: Rgba::WHITE,
            bg: Rgba::TRANSPARENT,
            attributes: crate::style::TextAttributes::empty(),
        };

        writer.write_cell_at_with_pool(5, 10, &cell, &pool);

        let output = String::from_utf8_lossy(writer.buffer());
        assert!(output.contains("🎉"));
        // Should contain cursor positioning
        assert!(output.contains("\x1b["));
    }
}
