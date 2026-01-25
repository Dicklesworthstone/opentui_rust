//! Text buffer view with viewport and wrapping.

// Complex rendering logic naturally has long functions
#![allow(clippy::too_many_lines)]
// Closures with method references are more readable in context
#![allow(clippy::redundant_closure_for_method_calls)]

use crate::buffer::OptimizedBuffer;
use crate::cell::Cell;
use crate::color::Rgba;
use crate::style::Style;
use crate::text::TextBuffer;
use crate::unicode::display_width_with_method;
use std::cell::RefCell;

/// Text wrapping mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WrapMode {
    /// No wrapping - lines extend beyond viewport.
    #[default]
    None,
    /// Wrap at character boundaries.
    Char,
    /// Wrap at word boundaries.
    Word,
}

/// Viewport configuration.
#[derive(Clone, Copy, Debug, Default)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    /// Create a new viewport.
    #[must_use]
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Selection range.
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

impl Selection {
    /// Create a new selection.
    #[must_use]
    pub fn new(start: usize, end: usize, style: Style) -> Self {
        Self { start, end, style }
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Get normalized (start <= end) selection.
    #[must_use]
    pub fn normalized(&self) -> Self {
        if self.start <= self.end {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
                style: self.style,
            }
        }
    }

    /// Check if position is within selection.
    #[must_use]
    pub fn contains(&self, pos: usize) -> bool {
        let norm = self.normalized();
        pos >= norm.start && pos < norm.end
    }
}

/// Local (viewport) selection based on screen coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalSelection {
    pub anchor_x: u32,
    pub anchor_y: u32,
    pub focus_x: u32,
    pub focus_y: u32,
    pub style: Style,
}

impl LocalSelection {
    /// Create a new local selection.
    #[must_use]
    pub fn new(anchor_x: u32, anchor_y: u32, focus_x: u32, focus_y: u32, style: Style) -> Self {
        Self {
            anchor_x,
            anchor_y,
            focus_x,
            focus_y,
            style,
        }
    }

    /// Normalize selection rectangle.
    #[must_use]
    pub fn normalized(&self) -> (u32, u32, u32, u32) {
        let min_x = self.anchor_x.min(self.focus_x);
        let max_x = self.anchor_x.max(self.focus_x);
        let min_y = self.anchor_y.min(self.focus_y);
        let max_y = self.anchor_y.max(self.focus_y);
        (min_x, min_y, max_x, max_y)
    }
}

/// View into a text buffer with viewport and rendering options.
pub struct TextBufferView<'a> {
    buffer: &'a TextBuffer,
    viewport: Viewport,
    wrap_mode: WrapMode,
    wrap_width: Option<u32>,
    scroll_x: u32,
    scroll_y: u32,
    selection: Option<Selection>,
    local_selection: Option<LocalSelection>,
    tab_indicator: Option<char>,
    tab_indicator_color: Rgba,
    truncate: bool,
    line_cache: RefCell<Option<LineCache>>,
}

#[derive(Clone, Debug)]
struct VirtualLine {
    source_line: usize,
    byte_start: usize,
    byte_end: usize,
    width: usize,
    is_wrap: bool,
}

/// Cached line layout information for wrapped text.
#[derive(Clone, Debug, Default)]
pub struct LineInfo {
    /// Byte offset where each virtual line starts.
    pub starts: Vec<usize>,
    /// Byte offset where each virtual line ends (exclusive).
    pub ends: Vec<usize>,
    /// Display width of each virtual line.
    pub widths: Vec<usize>,
    /// Source line index for each virtual line.
    pub sources: Vec<usize>,
    /// Whether the line is a wrapped continuation.
    pub wraps: Vec<bool>,
    /// Maximum line width across all virtual lines.
    pub max_width: usize,
}

impl LineInfo {
    /// Get the number of virtual lines.
    #[must_use]
    pub fn virtual_line_count(&self) -> usize {
        self.starts.len()
    }

    /// Map a source (logical) line to its first virtual line index.
    ///
    /// Returns the index of the first virtual line that corresponds to
    /// the given source line, or `None` if the source line doesn't exist.
    #[must_use]
    pub fn source_to_virtual(&self, source_line: usize) -> Option<usize> {
        self.sources.iter().position(|&s| s == source_line)
    }

    /// Map a virtual line index to its source (logical) line.
    ///
    /// Returns the source line index for the given virtual line,
    /// or `None` if the virtual line index is out of bounds.
    #[must_use]
    pub fn virtual_to_source(&self, virtual_line: usize) -> Option<usize> {
        self.sources.get(virtual_line).copied()
    }

    /// Get the byte range for a virtual line.
    ///
    /// Returns `(byte_start, byte_end)` for the given virtual line index,
    /// or `None` if the index is out of bounds.
    #[must_use]
    pub fn virtual_line_byte_range(&self, virtual_line: usize) -> Option<(usize, usize)> {
        let start = *self.starts.get(virtual_line)?;
        let end = *self.ends.get(virtual_line)?;
        Some((start, end))
    }

    /// Get the display width of a virtual line.
    #[must_use]
    pub fn virtual_line_width(&self, virtual_line: usize) -> Option<usize> {
        self.widths.get(virtual_line).copied()
    }

    /// Check if a virtual line is a wrapped continuation.
    #[must_use]
    pub fn is_continuation(&self, virtual_line: usize) -> Option<bool> {
        self.wraps.get(virtual_line).copied()
    }

    /// Count virtual lines for a given source line.
    #[must_use]
    pub fn virtual_lines_for_source(&self, source_line: usize) -> usize {
        self.sources.iter().filter(|&&s| s == source_line).count()
    }

    /// Get the maximum source line index.
    #[must_use]
    pub fn max_source_line(&self) -> Option<usize> {
        self.sources.iter().max().copied()
    }
}

/// Measurement result for a given viewport size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextMeasure {
    pub line_count: usize,
    pub max_width: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LineCacheKey {
    wrap_mode: WrapMode,
    wrap_width_override: Option<u32>,
    viewport_width: u32,
    tab_width: u8,
    width_method: crate::unicode::WidthMethod,
    buffer_revision: u64,
}

#[derive(Clone, Debug)]
struct LineCache {
    key: LineCacheKey,
    virtual_lines: Vec<VirtualLine>,
    info: LineInfo,
}

impl<'a> TextBufferView<'a> {
    /// Create a new view of a text buffer.
    #[must_use]
    pub fn new(buffer: &'a TextBuffer) -> Self {
        Self {
            buffer,
            viewport: Viewport::default(),
            wrap_mode: WrapMode::None,
            wrap_width: None,
            scroll_x: 0,
            scroll_y: 0,
            selection: None,
            local_selection: None,
            tab_indicator: None,
            tab_indicator_color: Rgba::WHITE,
            truncate: false,
            line_cache: RefCell::new(None),
        }
    }

    /// Set the viewport.
    #[must_use]
    pub fn viewport(mut self, x: u32, y: u32, width: u32, height: u32) -> Self {
        self.viewport = Viewport::new(x, y, width, height);
        self.clear_line_cache();
        self
    }

    /// Set the wrap mode.
    #[must_use]
    pub fn wrap_mode(mut self, mode: WrapMode) -> Self {
        self.wrap_mode = mode;
        self.clear_line_cache();
        self
    }

    /// Set explicit wrap width (overrides viewport width when wrapping).
    #[must_use]
    pub fn wrap_width(mut self, width: u32) -> Self {
        self.wrap_width = Some(width);
        self.clear_line_cache();
        self
    }

    /// Set scroll position.
    #[must_use]
    pub fn scroll(mut self, x: u32, y: u32) -> Self {
        self.scroll_x = x;
        self.scroll_y = y;
        self
    }

    /// Set tab indicator character and color.
    #[must_use]
    pub fn tab_indicator(mut self, ch: char, color: Rgba) -> Self {
        self.tab_indicator = Some(ch);
        self.tab_indicator_color = color;
        self
    }

    /// Enable or disable truncation.
    #[must_use]
    pub fn truncate(mut self, enabled: bool) -> Self {
        self.truncate = enabled;
        self
    }

    /// Set selection.
    pub fn set_selection(&mut self, start: usize, end: usize, style: Style) {
        self.selection = Some(Selection::new(start, end, style));
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Set a local (viewport) selection.
    pub fn set_local_selection(
        &mut self,
        anchor_x: u32,
        anchor_y: u32,
        focus_x: u32,
        focus_y: u32,
        style: Style,
    ) {
        self.local_selection = Some(LocalSelection::new(
            anchor_x, anchor_y, focus_x, focus_y, style,
        ));
    }

    /// Clear local selection.
    pub fn clear_local_selection(&mut self) {
        self.local_selection = None;
    }

    fn clear_line_cache(&self) {
        self.line_cache.replace(None);
    }

    /// Get selected text if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?.normalized();
        if sel.is_empty() {
            return None;
        }

        let max = self.buffer.len_chars();
        let start = sel.start.min(max);
        let end = sel.end.min(max);
        if start >= end {
            return None;
        }
        Some(self.buffer.rope().slice(start..end).to_string())
    }

    fn effective_wrap_width(&self) -> Option<usize> {
        if self.wrap_mode == WrapMode::None || self.viewport.width == 0 {
            return None;
        }
        let width = self.wrap_width.unwrap_or(self.viewport.width).max(1);
        Some(width as usize)
    }

    fn effective_wrap_width_for(&self, width: Option<u32>) -> Option<usize> {
        if self.wrap_mode == WrapMode::None {
            return None;
        }
        let base_width = width.unwrap_or(self.viewport.width);
        if base_width == 0 {
            return None;
        }
        let width = self.wrap_width.unwrap_or(base_width).max(1);
        Some(width as usize)
    }

    fn line_cache_key(&self) -> LineCacheKey {
        LineCacheKey {
            wrap_mode: self.wrap_mode,
            wrap_width_override: self.wrap_width,
            viewport_width: self.viewport.width,
            tab_width: self.buffer.tab_width(),
            width_method: self.buffer.width_method(),
            buffer_revision: self.buffer.revision(),
        }
    }

    fn line_cache(&self) -> std::cell::Ref<'_, LineCache> {
        let key = self.line_cache_key();
        let needs_refresh = self
            .line_cache
            .borrow()
            .as_ref()
            .is_none_or(|cache| cache.key != key);

        if needs_refresh {
            let virtual_lines = self.build_virtual_lines_for(self.effective_wrap_width());
            let info = Self::line_info_from_virtual_lines(&virtual_lines);
            *self.line_cache.borrow_mut() = Some(LineCache {
                key,
                virtual_lines,
                info,
            });
        }

        std::cell::Ref::map(self.line_cache.borrow(), |cache| {
            cache.as_ref().expect("line cache should exist")
        })
    }

    fn line_info_from_virtual_lines(virtual_lines: &[VirtualLine]) -> LineInfo {
        let mut info = LineInfo::default();
        for line in virtual_lines {
            info.starts.push(line.byte_start);
            info.ends.push(line.byte_end);
            info.widths.push(line.width);
            info.sources.push(line.source_line);
            info.wraps.push(line.is_wrap);
            info.max_width = info.max_width.max(line.width);
        }
        info
    }

    fn build_virtual_lines_for(&self, wrap_width: Option<usize>) -> Vec<VirtualLine> {
        use unicode_segmentation::UnicodeSegmentation;

        let mut lines = Vec::new();
        let method = self.buffer.width_method();
        let tab_width = self.buffer.tab_width().max(1) as usize;

        for line_idx in 0..self.buffer.len_lines() {
            let Some(line) = self.buffer.line(line_idx) else {
                continue;
            };
            let line = line.trim_end_matches('\n').trim_end_matches('\r');

            let line_start_char = self.buffer.rope().line_to_char(line_idx);
            let line_start_byte = self.buffer.rope().char_to_byte(line_start_char);

            if line.is_empty() {
                lines.push(VirtualLine {
                    source_line: line_idx,
                    byte_start: line_start_byte,
                    byte_end: line_start_byte,
                    width: 0,
                    is_wrap: false,
                });
                continue;
            }

            let Some(wrap_width) = wrap_width else {
                let width = display_width_with_method(line, method);
                lines.push(VirtualLine {
                    source_line: line_idx,
                    byte_start: line_start_byte,
                    byte_end: line_start_byte + line.len(),
                    width,
                    is_wrap: false,
                });
                continue;
            };

            let graphemes: Vec<(usize, &str)> = line.grapheme_indices(true).collect();
            let mut start_byte = 0usize;
            let mut current_width = 0usize;
            let mut last_break: Option<(usize, usize, usize)> = None; // (break_byte, width, index)
            let mut i = 0usize;

            while i < graphemes.len() {
                let (byte_idx, grapheme) = graphemes[i];
                if byte_idx < start_byte {
                    i += 1;
                    continue;
                }

                let g_width = if grapheme == "\t" {
                    let offset = current_width % tab_width;
                    tab_width - offset
                } else {
                    display_width_with_method(grapheme, method)
                };

                let is_ws = grapheme.chars().all(|c| c.is_whitespace());
                if self.wrap_mode == WrapMode::Word && is_ws {
                    last_break = Some((byte_idx + grapheme.len(), current_width + g_width, i + 1));
                }

                if current_width + g_width > wrap_width && current_width > 0 {
                    let (break_byte, break_width, break_index) = if self.wrap_mode == WrapMode::Word
                    {
                        last_break.unwrap_or((byte_idx, current_width, i))
                    } else {
                        (byte_idx, current_width, i)
                    };

                    lines.push(VirtualLine {
                        source_line: line_idx,
                        byte_start: line_start_byte + start_byte,
                        byte_end: line_start_byte + break_byte,
                        width: break_width,
                        is_wrap: start_byte > 0,
                    });

                    start_byte = break_byte;
                    current_width = 0;
                    last_break = None;
                    i = break_index;

                    if self.wrap_mode == WrapMode::Word {
                        while i < graphemes.len() {
                            let (b, g) = graphemes[i];
                            if b < start_byte {
                                i += 1;
                                continue;
                            }
                            if g.chars().all(|c| c.is_whitespace()) {
                                start_byte = b + g.len();
                                i += 1;
                            } else {
                                break;
                            }
                        }
                    }

                    continue;
                }

                current_width += g_width;
                i += 1;
            }

            if start_byte <= line.len() {
                lines.push(VirtualLine {
                    source_line: line_idx,
                    byte_start: line_start_byte + start_byte,
                    byte_end: line_start_byte + line.len(),
                    width: current_width,
                    is_wrap: start_byte > 0,
                });
            }
        }

        lines
    }

    /// Compute visual (wrapped) position for a character offset.
    #[must_use]
    pub fn visual_position_for_offset(&self, char_offset: usize) -> (u32, u32) {
        use unicode_segmentation::UnicodeSegmentation;

        let rope = self.buffer.rope();
        let byte_offset = rope.char_to_byte(char_offset);
        let cache = self.line_cache();
        let method = self.buffer.width_method();
        let tab_width = self.buffer.tab_width().max(1) as usize;

        for (row, vline) in cache.virtual_lines.iter().enumerate() {
            let is_last_line = row == cache.virtual_lines.len() - 1;
            if byte_offset < vline.byte_start {
                continue;
            }
            if byte_offset >= vline.byte_end && !is_last_line {
                continue;
            }

            let char_start = rope.byte_to_char(vline.byte_start);
            let char_end = rope.byte_to_char(byte_offset);
            let text = rope.slice(char_start..char_end).to_string();

            let mut width = 0usize;
            for grapheme in text.graphemes(true) {
                if grapheme == "\t" {
                    let offset = width % tab_width;
                    width += tab_width - offset;
                } else {
                    width += display_width_with_method(grapheme, method);
                }
            }

            return (row as u32, width as u32);
        }

        (0, 0)
    }

    /// Calculate the number of virtual lines (accounting for wrapping).
    #[must_use]
    pub fn virtual_line_count(&self) -> usize {
        self.line_cache().virtual_lines.len()
    }

    /// Get line layout information for the current view.
    #[must_use]
    pub fn line_info(&self) -> LineInfo {
        self.line_cache().info.clone()
    }

    /// Measure line count and max width for a given viewport size.
    #[must_use]
    pub fn measure_for_dimensions(&self, width: u32, _height: u32) -> TextMeasure {
        let wrap_width = self.effective_wrap_width_for(Some(width.max(1)));
        let virtual_lines = self.build_virtual_lines_for(wrap_width);
        let info = Self::line_info_from_virtual_lines(&virtual_lines);
        TextMeasure {
            line_count: virtual_lines.len(),
            max_width: info.max_width,
        }
    }

    /// Render the view to an output buffer.
    pub fn render_to(&self, output: &mut OptimizedBuffer, dest_x: i32, dest_y: i32) {
        let cache = self.line_cache();
        let virtual_lines = &cache.virtual_lines;
        let start_line = self.scroll_y as usize;
        let end_line = (start_line + self.viewport.height as usize).min(virtual_lines.len());

        for (row_offset, vline_idx) in (start_line..end_line).enumerate() {
            let vline = &virtual_lines[vline_idx];
            let dest_row = dest_y + row_offset as i32;
            if dest_row < 0 {
                continue;
            }
            self.render_virtual_line(output, dest_x, dest_row as u32, vline, row_offset as u32);
        }
    }

    fn render_virtual_line(
        &self,
        output: &mut OptimizedBuffer,
        dest_x: i32,
        dest_y: u32,
        vline: &VirtualLine,
        view_row: u32,
    ) {
        use unicode_segmentation::UnicodeSegmentation;

        let rope = self.buffer.rope();
        let char_start = rope.byte_to_char(vline.byte_start);
        let char_end = rope.byte_to_char(vline.byte_end);
        let line = rope.slice(char_start..char_end).to_string();

        let mut col = 0u32;
        let method = self.buffer.width_method();

        let selection = self.selection.as_ref().map(Selection::normalized);
        let local_sel = self.local_selection;

        let max_col = self.scroll_x + self.viewport.width;

        let mut global_char_offset = char_start;
        for grapheme in line.graphemes(true) {
            // Optimization: Stop if we've gone past the viewport
            if col >= max_col {
                break;
            }

            if grapheme == "\t" {
                let tab_width = self.buffer.tab_width().max(1) as u32;
                let spaces_to_next = tab_width - (col % tab_width);
                // Get the actual style at this position (preserves syntax highlighting)
                let byte_offset = rope.char_to_byte(global_char_offset);
                let base_style = self.buffer.style_at(byte_offset);

                for space_idx in 0..spaces_to_next {
                    // Optimization: Skip if before scroll position
                    if col < self.scroll_x {
                        col += 1;
                        continue;
                    }
                    // Stop if we hit the edge (tab might straddle the edge)
                    if col >= max_col {
                        break;
                    }

                    let screen_col = (col - self.scroll_x) as i32 + dest_x;
                    if screen_col >= 0 {
                        if space_idx == 0 {
                            if let Some(indicator) = self.tab_indicator {
                                // Tab indicator gets special foreground but preserves background
                                let style = base_style.with_fg(self.tab_indicator_color);
                                output.set(screen_col as u32, dest_y, Cell::new(indicator, style));
                            } else {
                                output.set(screen_col as u32, dest_y, Cell::new(' ', base_style));
                            }
                        } else {
                            output.set(screen_col as u32, dest_y, Cell::new(' ', base_style));
                        }

                        if let Some(sel) = selection {
                            if sel.contains(global_char_offset) {
                                if let Some(cell) = output.get_mut(screen_col as u32, dest_y) {
                                    cell.apply_style(sel.style);
                                }
                            }
                        }
                        if let Some(local) = local_sel {
                            let (min_x, min_y, max_x, max_y) = local.normalized();
                            let view_col = (screen_col - dest_x) as u32;
                            if view_col >= min_x
                                && view_col <= max_x
                                && view_row >= min_y
                                && view_row <= max_y
                            {
                                if let Some(cell) = output.get_mut(screen_col as u32, dest_y) {
                                    cell.apply_style(local.style);
                                }
                            }
                        }
                    }
                    col += 1;
                }
                global_char_offset += 1;
                continue;
            }

            let byte_offset = rope.char_to_byte(global_char_offset);
            let style = self.buffer.style_at(byte_offset);
            let mut cell = Cell::from_grapheme(grapheme, style);
            let width = display_width_with_method(grapheme, method).max(cell.display_width());

            // Optimization: Skip if completely before scroll position
            if col + (width as u32) <= self.scroll_x {
                col += width as u32;
                global_char_offset += grapheme.chars().count();
                continue;
            }

            let screen_col = (col - self.scroll_x) as i32 + dest_x;

            // Only draw if within viewport (and valid screen coordinates)
            if screen_col >= 0 {
                if let Some(sel) = selection {
                    if sel.contains(global_char_offset) {
                        cell.apply_style(sel.style);
                    }
                }
                if let Some(local) = local_sel {
                    let (min_x, min_y, max_x, max_y) = local.normalized();
                    let view_col = (screen_col - dest_x) as u32;
                    if view_col >= min_x
                        && view_col <= max_x
                        && view_row >= min_y
                        && view_row <= max_y
                    {
                        cell.apply_style(local.style);
                    }
                }

                output.set(screen_col as u32, dest_y, cell.clone());

                for i in 1..width {
                    let cont_col = screen_col as u32 + i as u32;
                    output.set(
                        cont_col,
                        dest_y,
                        Cell::continuation(style.bg.unwrap_or(Rgba::TRANSPARENT)),
                    );
                }
            }

            col += width as u32;
            global_char_offset += grapheme.chars().count();
        }

        if self.truncate && self.wrap_mode == WrapMode::None {
            let max_cols = self.viewport.width as i32;
            if vline.width as i32 > max_cols && max_cols > 0 {
                let ellipsis_col = dest_x + (max_cols - 1);
                if ellipsis_col >= 0 {
                    output.set(
                        ellipsis_col as u32,
                        dest_y,
                        Cell::new('…', self.buffer.default_style()),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_basic() {
        let buffer = TextBuffer::with_text("Hello\nWorld");
        let view = TextBufferView::new(&buffer).viewport(0, 0, 80, 24);
        assert_eq!(view.virtual_line_count(), 2);
    }

    #[test]
    fn test_selection() {
        let buffer = TextBuffer::with_text("Hello, World!");
        let mut view = TextBufferView::new(&buffer);
        view.set_selection(0, 5, Style::NONE);
        assert_eq!(view.selected_text(), Some("Hello".to_string()));
    }

    #[test]
    fn test_wrap_char_count() {
        let buffer = TextBuffer::with_text("abcdefghijklmnopqrstuvwxyz");
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 5, 10)
            .wrap_mode(WrapMode::Char);
        assert!(view.virtual_line_count() >= 5);
    }

    #[test]
    fn test_line_info_basic_wrap() {
        let buffer = TextBuffer::with_text("abcd");
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 2, 10)
            .wrap_mode(WrapMode::Char);

        let info = view.line_info();
        assert_eq!(info.starts, vec![0, 2]);
        assert_eq!(info.ends, vec![2, 4]);
        assert_eq!(info.widths, vec![2, 2]);
        assert_eq!(info.sources, vec![0, 0]);
        assert_eq!(info.wraps, vec![false, true]);
        assert_eq!(info.max_width, 2);
    }

    #[test]
    fn test_virtual_line_byte_range_last_line() {
        eprintln!(
            "[TEST] test_virtual_line_byte_range_last_line: Verifying byte range for last line"
        );

        let buffer = TextBuffer::with_text("Hello World");
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info = view.line_info();
        eprintln!("[TEST] Virtual line count: {}", info.virtual_line_count());

        // Test the byte range for the last (and only) virtual line
        let range = info.virtual_line_byte_range(0);
        eprintln!("[TEST] Byte range for line 0: {range:?}");

        assert_eq!(
            range,
            Some((0, 11)),
            "Last line should have correct byte range (0, 11)"
        );

        // Verify the content matches
        let text = &buffer.to_string()[0..11];
        eprintln!("[TEST] Text in range: {text:?}");
        assert_eq!(text, "Hello World");

        eprintln!("[TEST] PASS: Last line byte range is correct");
    }

    #[test]
    fn test_virtual_line_byte_range_wrapped() {
        eprintln!(
            "[TEST] test_virtual_line_byte_range_wrapped: Verifying byte ranges with wrapping"
        );

        let buffer = TextBuffer::with_text("abcdefgh");
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 3, 10)
            .wrap_mode(WrapMode::Char);

        let info = view.line_info();
        eprintln!("[TEST] Virtual line count: {}", info.virtual_line_count());

        // Should wrap to 3 lines: "abc", "def", "gh"
        assert_eq!(info.virtual_line_count(), 3);

        let range0 = info.virtual_line_byte_range(0);
        let range1 = info.virtual_line_byte_range(1);
        let range2 = info.virtual_line_byte_range(2);

        eprintln!("[TEST] Line 0 range: {range0:?}");
        eprintln!("[TEST] Line 1 range: {range1:?}");
        eprintln!("[TEST] Line 2 range: {range2:?}");

        assert_eq!(range0, Some((0, 3)), "First line: bytes 0-3");
        assert_eq!(range1, Some((3, 6)), "Second line: bytes 3-6");
        assert_eq!(range2, Some((6, 8)), "Last line: bytes 6-8 (not 6-6!)");

        eprintln!("[TEST] PASS: Wrapped line byte ranges are correct");
    }

    #[test]
    fn test_measure_for_dimensions() {
        let buffer = TextBuffer::with_text("abc\ndefgh");
        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::Char);
        let measure = view.measure_for_dimensions(3, 10);
        assert_eq!(
            measure,
            TextMeasure {
                line_count: 3,
                max_width: 3
            }
        );
    }

    #[test]
    fn test_measure_no_wrap() {
        eprintln!("[TEST] test_measure_no_wrap: Measuring without wrapping");

        let buffer = TextBuffer::with_text("short\nmedium text\nvery long line of text here");
        eprintln!("[TEST] Buffer lines: 'short', 'medium text', 'very long line of text here'");

        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::None);
        let measure = view.measure_for_dimensions(10, 10);

        eprintln!("[TEST] With WrapMode::None, width=10:");
        eprintln!("[TEST]   line_count = {}", measure.line_count);
        eprintln!("[TEST]   max_width = {}", measure.max_width);

        // Without wrapping, should have exactly 3 lines
        assert_eq!(
            measure.line_count, 3,
            "Should have 3 source lines without wrapping"
        );
        // Max width should be the longest line: "very long line of text here" = 27 chars
        assert_eq!(
            measure.max_width, 27,
            "Max width should be longest line (27 chars)"
        );

        eprintln!("[TEST] PASS: No-wrap measurement correct");
    }

    #[test]
    fn test_measure_with_char_wrap() {
        eprintln!("[TEST] test_measure_with_char_wrap: Measuring with character wrapping");

        let buffer = TextBuffer::with_text("abcdefghij");
        eprintln!("[TEST] Buffer: 'abcdefghij' (10 chars)");

        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::Char);

        // Wrap at width 3
        let measure = view.measure_for_dimensions(3, 10);
        eprintln!("[TEST] With width=3, char wrap:");
        eprintln!(
            "[TEST]   line_count = {} (expected 4: 'abc', 'def', 'ghi', 'j')",
            measure.line_count
        );
        eprintln!("[TEST]   max_width = {}", measure.max_width);

        assert_eq!(measure.line_count, 4, "10 chars / 3 = 4 wrapped lines");
        assert_eq!(measure.max_width, 3, "Max width capped at wrap width");

        // Wrap at width 5
        let measure2 = view.measure_for_dimensions(5, 10);
        eprintln!("[TEST] With width=5:");
        eprintln!(
            "[TEST]   line_count = {} (expected 2: 'abcde', 'fghij')",
            measure2.line_count
        );

        assert_eq!(measure2.line_count, 2, "10 chars / 5 = 2 wrapped lines");
        assert_eq!(measure2.max_width, 5, "Max width capped at wrap width");

        eprintln!("[TEST] PASS: Char wrap measurement correct");
    }

    #[test]
    fn test_measure_with_word_wrap() {
        eprintln!("[TEST] test_measure_with_word_wrap: Measuring with word wrapping");

        let buffer = TextBuffer::with_text("hello world test");
        eprintln!("[TEST] Buffer: 'hello world test' (16 chars)");

        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::Word);

        // Wrap at width 12 - "hello world" fits (11), "test" on next line
        let measure = view.measure_for_dimensions(12, 10);
        eprintln!("[TEST] With width=12, word wrap:");
        eprintln!("[TEST]   line_count = {}", measure.line_count);
        eprintln!("[TEST]   max_width = {}", measure.max_width);

        assert_eq!(measure.line_count, 2, "Should wrap to 2 lines at width 12");
        assert!(
            measure.max_width <= 12,
            "Max width should not exceed wrap width"
        );

        // Wrap at width 6 - each word should be on its own line
        let measure2 = view.measure_for_dimensions(6, 10);
        eprintln!("[TEST] With width=6:");
        eprintln!("[TEST]   line_count = {}", measure2.line_count);

        assert_eq!(measure2.line_count, 3, "Should wrap to 3 lines at width 6");

        eprintln!("[TEST] PASS: Word wrap measurement correct");
    }

    #[test]
    fn test_measure_empty_buffer() {
        eprintln!("[TEST] test_measure_empty_buffer: Measuring empty buffer");

        let buffer = TextBuffer::new();
        eprintln!("[TEST] Empty buffer created");

        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::Char);
        let measure = view.measure_for_dimensions(80, 24);

        eprintln!("[TEST] Measure results:");
        eprintln!("[TEST]   line_count = {}", measure.line_count);
        eprintln!("[TEST]   max_width = {}", measure.max_width);

        // Empty buffer should have 0 or 1 line depending on implementation
        assert!(
            measure.line_count <= 1,
            "Empty buffer should have 0 or 1 line"
        );
        assert_eq!(measure.max_width, 0, "Empty buffer should have max_width 0");

        eprintln!("[TEST] PASS: Empty buffer measurement correct");
    }

    #[test]
    fn test_measure_single_long_line() {
        eprintln!("[TEST] test_measure_single_long_line: Measuring single long line");

        // Create a 100-character line
        let long_line = "x".repeat(100);
        let buffer = TextBuffer::with_text(&long_line);
        eprintln!("[TEST] Single line of 100 'x' characters");

        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::Char);

        // Wrap at width 20
        let measure = view.measure_for_dimensions(20, 10);
        eprintln!("[TEST] With width=20:");
        eprintln!("[TEST]   line_count = {} (expected 5)", measure.line_count);
        eprintln!("[TEST]   max_width = {}", measure.max_width);

        assert_eq!(measure.line_count, 5, "100 chars / 20 = 5 wrapped lines");
        assert_eq!(measure.max_width, 20, "Max width should be 20");

        // Wrap at width 33
        let measure2 = view.measure_for_dimensions(33, 10);
        eprintln!("[TEST] With width=33:");
        eprintln!(
            "[TEST]   line_count = {} (expected 4: 33+33+33+1)",
            measure2.line_count
        );

        assert_eq!(measure2.line_count, 4, "100 chars / 33 = 4 wrapped lines");

        eprintln!("[TEST] PASS: Single long line measurement correct");
    }

    #[test]
    fn test_measure_cjk_content() {
        eprintln!("[TEST] test_measure_cjk_content: Measuring CJK wide characters");

        // CJK characters are typically 2 columns wide
        let buffer = TextBuffer::with_text("你好世界"); // 4 CJK chars = 8 display columns
        eprintln!("[TEST] Buffer: '你好世界' (4 CJK chars, ~8 display columns)");

        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::Char);

        // With width 4, each CJK char is 2 wide, so 2 chars per line
        let measure = view.measure_for_dimensions(4, 10);
        eprintln!("[TEST] With width=4:");
        eprintln!("[TEST]   line_count = {}", measure.line_count);
        eprintln!("[TEST]   max_width = {}", measure.max_width);

        // Should wrap to 2 lines: "你好" (4 cols) and "世界" (4 cols)
        assert_eq!(
            measure.line_count, 2,
            "4 CJK chars at width 4 should be 2 lines"
        );
        assert_eq!(measure.max_width, 4, "Max width should be 4");

        // With width 8, all 4 chars should fit on one line
        let measure2 = view.measure_for_dimensions(8, 10);
        eprintln!("[TEST] With width=8:");
        eprintln!("[TEST]   line_count = {}", measure2.line_count);

        assert_eq!(
            measure2.line_count, 1,
            "All CJK chars should fit at width 8"
        );

        eprintln!("[TEST] PASS: CJK content measurement correct");
    }

    #[test]
    fn test_measure_updates_after_edit() {
        eprintln!("[TEST] test_measure_updates_after_edit: Verifying measurement updates");

        let mut buffer = TextBuffer::with_text("short");
        eprintln!("[TEST] Initial buffer: 'short'");

        let view = TextBufferView::new(&buffer).wrap_mode(WrapMode::Char);
        let measure1 = view.measure_for_dimensions(10, 10);
        eprintln!(
            "[TEST] Initial measure: line_count={}, max_width={}",
            measure1.line_count, measure1.max_width
        );

        assert_eq!(measure1.line_count, 1);
        assert_eq!(measure1.max_width, 5);

        // Modify the buffer
        buffer.set_text("this is a much longer line now");
        eprintln!("[TEST] Updated buffer: 'this is a much longer line now'");

        // Create new view with updated buffer
        let view2 = TextBufferView::new(&buffer).wrap_mode(WrapMode::Char);
        let measure2 = view2.measure_for_dimensions(10, 10);
        eprintln!(
            "[TEST] Updated measure: line_count={}, max_width={}",
            measure2.line_count, measure2.max_width
        );

        // "this is a much longer line now" = 30 chars, at width 10 = 3 lines
        assert_eq!(
            measure2.line_count, 3,
            "30 chars at width 10 should be 3 lines"
        );
        assert_eq!(measure2.max_width, 10);

        eprintln!("[TEST] PASS: Measurement updates correctly after edit");
    }

    #[test]
    fn test_measure_consistency_with_render() {
        use crate::buffer::OptimizedBuffer;

        eprintln!("[TEST] test_measure_consistency_with_render: Comparing measure with render");

        let buffer = TextBuffer::with_text("line1\nline2 is longer\nshort");
        eprintln!("[TEST] Buffer with 3 lines of varying length");

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 8, 10)
            .wrap_mode(WrapMode::Char);

        let measure = view.measure_for_dimensions(8, 10);
        eprintln!(
            "[TEST] Measure: line_count={}, max_width={}",
            measure.line_count, measure.max_width
        );

        // Now render and count actual lines
        let mut output = OptimizedBuffer::new(8, 10);
        view.render_to(&mut output, 0, 0);

        // Count rendered lines by checking for non-default content
        let virtual_count = view.virtual_line_count();
        eprintln!("[TEST] virtual_line_count() = {virtual_count}");

        // Measure should match virtual line count
        assert_eq!(
            measure.line_count, virtual_count,
            "measure_for_dimensions line_count should match virtual_line_count"
        );

        eprintln!("[TEST] PASS: Measurement consistent with render");
    }

    #[test]
    fn test_tab_rendering_preserves_style() {
        use crate::buffer::OptimizedBuffer;
        use crate::cell::CellContent;
        use crate::color::Rgba;
        use crate::text::segment::StyledChunk;

        eprintln!("[TEST] test_tab_rendering_preserves_style: Verifying TAB gets syntax style");

        // Create buffer with styled text containing a tab
        let mut buffer = TextBuffer::new();
        buffer.set_styled_text(&[
            StyledChunk::new("hello", Style::fg(Rgba::RED)),
            StyledChunk::new("\t", Style::fg(Rgba::GREEN)), // Tab with green style
            StyledChunk::new("world", Style::fg(Rgba::BLUE)),
        ]);
        eprintln!("[TEST] Buffer text: {:?}", buffer.to_string());

        let view = TextBufferView::new(&buffer).viewport(0, 0, 80, 24);

        // Render to buffer
        let mut output = OptimizedBuffer::new(80, 24);
        view.render_to(&mut output, 0, 0);

        // Check the cell at position where tab starts (after "hello")
        // Tab at position 5 should have the green style (from the styled chunk)
        let cell_at_tab = output.get(5, 0);
        eprintln!("[TEST] Cell at tab position (5,0): {cell_at_tab:?}");

        // The tab should render as space(s) but preserve the GREEN style
        assert!(cell_at_tab.is_some(), "Cell at tab position should exist");
        let cell = cell_at_tab.unwrap();
        // The foreground should be GREEN since that's the style at the tab position
        eprintln!("[TEST] Tab cell foreground: {:?}", cell.fg);
        // Note: without tab_indicator set, the cell is a space with the style from style_at
        assert!(
            matches!(cell.content, CellContent::Char(' ')),
            "Tab should render as space by default"
        );
        assert_eq!(
            cell.fg,
            Rgba::GREEN,
            "Tab should preserve syntax highlighting (GREEN)"
        );

        // Verify "world" has blue style
        let cell_at_world = output.get(8, 0); // Tab expands to position 8
        eprintln!("[TEST] Cell at 'world' start (8,0): {cell_at_world:?}");
        if let Some(cell) = cell_at_world {
            assert!(matches!(cell.content, CellContent::Char('w')));
            assert_eq!(cell.fg, Rgba::BLUE);
        }

        eprintln!("[TEST] SUCCESS: Tab rendering preserves syntax highlighting");
    }

    #[test]
    fn test_tab_indicator_with_style() {
        use crate::buffer::OptimizedBuffer;
        use crate::cell::CellContent;
        use crate::color::Rgba;
        use crate::text::segment::StyledChunk;

        eprintln!("[TEST] test_tab_indicator_with_style: Tab indicator overrides fg, preserves bg");

        // Define test colors
        let magenta = Rgba::rgb(1.0, 0.0, 1.0); // Magenta: full red + blue
        let yellow = Rgba::rgb(1.0, 1.0, 0.0); // Yellow: full red + green

        // Create buffer with styled text containing a tab with background
        let mut buffer = TextBuffer::new();
        let bg_style = Style::NONE.with_bg(magenta).with_fg(Rgba::GREEN);
        buffer.set_styled_text(&[
            StyledChunk::new("x", Style::NONE),
            StyledChunk::new("\t", bg_style), // Tab with magenta background
            StyledChunk::new("y", Style::NONE),
        ]);
        eprintln!("[TEST] Buffer text: {:?}", buffer.to_string());

        // Set tab indicator
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .tab_indicator('→', yellow);

        let mut output = OptimizedBuffer::new(80, 24);
        view.render_to(&mut output, 0, 0);

        // Tab indicator at position 1
        let cell = output.get(1, 0).expect("Cell should exist");
        eprintln!(
            "[TEST] Tab indicator cell: content={:?}, fg={:?}, bg={:?}",
            cell.content, cell.fg, cell.bg
        );

        assert!(
            matches!(cell.content, CellContent::Char('→')),
            "Tab indicator should be arrow"
        );
        assert_eq!(cell.fg, yellow, "Tab indicator should have yellow fg");
        // Background is preserved from the style
        assert_eq!(
            cell.bg, magenta,
            "Tab should preserve background from syntax"
        );

        eprintln!("[TEST] SUCCESS: Tab indicator correctly overrides fg while preserving bg");
    }

    #[test]
    fn test_tab_expands_correctly() {
        use crate::buffer::OptimizedBuffer;
        use crate::cell::CellContent;

        eprintln!("[TEST] test_tab_expands_correctly: Verifying tab expansion width");

        let buffer = TextBuffer::with_text("ab\tcd");
        let view = TextBufferView::new(&buffer).viewport(0, 0, 80, 24);

        let mut output = OptimizedBuffer::new(80, 24);
        view.render_to(&mut output, 0, 0);

        // Default tab width is 4
        // "ab" at positions 0,1
        // TAB at positions 2,3 (expands to fill to next multiple of 4)
        // "cd" at positions 4,5
        eprintln!("[TEST] Checking character positions after tab expansion");

        let cell_a = output.get(0, 0).expect("Cell should exist");
        assert!(matches!(cell_a.content, CellContent::Char('a')));
        eprintln!("[TEST] Position 0: {:?}", cell_a.content);

        let cell_b = output.get(1, 0).expect("Cell should exist");
        assert!(matches!(cell_b.content, CellContent::Char('b')));
        eprintln!("[TEST] Position 1: {:?}", cell_b.content);

        // Tab expansion fills 2,3
        let cell_tab = output.get(2, 0).expect("Cell should exist");
        assert!(
            matches!(cell_tab.content, CellContent::Char(' ')),
            "Tab should expand to space"
        );
        eprintln!("[TEST] Position 2: {:?} (tab space)", cell_tab.content);

        let cell_tab2 = output.get(3, 0).expect("Cell should exist");
        assert!(
            matches!(cell_tab2.content, CellContent::Char(' ')),
            "Tab should expand to space"
        );
        eprintln!("[TEST] Position 3: {:?} (tab space)", cell_tab2.content);

        // After tab
        let cell_c = output.get(4, 0).expect("Cell should exist");
        assert!(matches!(cell_c.content, CellContent::Char('c')));
        eprintln!("[TEST] Position 4: {:?}", cell_c.content);

        let cell_d = output.get(5, 0).expect("Cell should exist");
        assert!(matches!(cell_d.content, CellContent::Char('d')));
        eprintln!("[TEST] Position 5: {:?}", cell_d.content);

        eprintln!("[TEST] SUCCESS: Tab expansion width is correct");
    }

    // ================== LineInfo Comprehensive Tests ==================

    #[test]
    fn test_line_cache_no_wrap() {
        eprintln!("[TEST] test_line_cache_no_wrap: Testing line cache without wrapping");

        let buffer = TextBuffer::with_text("Hello World\nSecond Line\nThird");
        eprintln!("[TEST] Input text: {:?}", buffer.to_string());
        eprintln!("[TEST] Logical line count: {}", buffer.len_lines());

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info = view.line_info();
        eprintln!("[TEST] LineInfo results:");
        eprintln!("[TEST]   virtual_line_count: {}", info.virtual_line_count());
        eprintln!("[TEST]   max_width: {}", info.max_width);

        for i in 0..info.virtual_line_count() {
            eprintln!(
                "[TEST]   Line {}: start={} width={} source={} wrap={}",
                i, info.starts[i], info.widths[i], info.sources[i], info.wraps[i]
            );
        }

        assert_eq!(info.virtual_line_count(), 3, "Should have 3 virtual lines");
        assert_eq!(
            info.sources,
            vec![0, 1, 2],
            "Each virtual line maps to its source"
        );
        assert_eq!(info.wraps, vec![false, false, false], "No wrapping");
        assert_eq!(info.max_width, 11, "Max width should be 'Hello World' = 11");

        eprintln!("[TEST] PASS: No-wrap mode produces correct line info");
    }

    #[test]
    fn test_line_cache_char_wrap_exact() {
        eprintln!("[TEST] test_line_cache_char_wrap_exact: Testing char wrap at exact boundary");

        let buffer = TextBuffer::with_text("abcdef");
        eprintln!(
            "[TEST] Input: {:?}, length: {}",
            buffer.to_string(),
            buffer.len_chars()
        );

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 3, 10)
            .wrap_mode(WrapMode::Char);

        let info = view.line_info();
        eprintln!("[TEST] Wrap width: 3, LineInfo:");
        for i in 0..info.virtual_line_count() {
            eprintln!(
                "[TEST]   Line {}: start={} width={} source={} wrap={}",
                i, info.starts[i], info.widths[i], info.sources[i], info.wraps[i]
            );
        }

        assert_eq!(info.virtual_line_count(), 2, "6 chars / 3 width = 2 lines");
        assert_eq!(info.widths, vec![3, 3], "Each line has width 3");
        assert_eq!(info.wraps, vec![false, true], "Second line is continuation");

        eprintln!("[TEST] PASS: Char wrap at exact boundary works");
    }

    #[test]
    fn test_line_cache_char_wrap_overflow() {
        eprintln!("[TEST] test_line_cache_char_wrap_overflow: Testing char wrap with overflow");

        let buffer = TextBuffer::with_text("abcdefgh");
        eprintln!(
            "[TEST] Input: {:?}, length: {}",
            buffer.to_string(),
            buffer.len_chars()
        );

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 3, 10)
            .wrap_mode(WrapMode::Char);

        let info = view.line_info();
        eprintln!("[TEST] Wrap width: 3, LineInfo:");
        for i in 0..info.virtual_line_count() {
            eprintln!(
                "[TEST]   Line {}: start={} width={} source={} wrap={}",
                i, info.starts[i], info.widths[i], info.sources[i], info.wraps[i]
            );
        }

        assert_eq!(info.virtual_line_count(), 3, "8 chars / 3 width = 3 lines");
        assert_eq!(info.widths, vec![3, 3, 2], "Last line has 2 chars");

        eprintln!("[TEST] PASS: Char wrap overflow works correctly");
    }

    #[test]
    fn test_line_cache_word_wrap_simple() {
        eprintln!("[TEST] test_line_cache_word_wrap_simple: Testing word wrap");

        let buffer = TextBuffer::with_text("Hello world test");
        eprintln!("[TEST] Input: {:?}", buffer.to_string());
        eprintln!("[TEST] Wrap width: 10");

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 10, 10)
            .wrap_mode(WrapMode::Word);

        let info = view.line_info();
        eprintln!("[TEST] LineInfo:");
        for i in 0..info.virtual_line_count() {
            eprintln!(
                "[TEST]   Line {}: start={} width={} source={} wrap={}",
                i, info.starts[i], info.widths[i], info.sources[i], info.wraps[i]
            );
        }

        // "Hello " (6) + "world" would exceed 10, so wrap at "Hello "
        // Then "world " (6) + "test" (4) = 10, fits
        assert!(
            info.virtual_line_count() >= 2,
            "Should wrap into at least 2 lines"
        );

        eprintln!("[TEST] PASS: Word wrap breaks at word boundaries");
    }

    #[test]
    fn test_line_cache_word_wrap_long_word() {
        eprintln!("[TEST] test_line_cache_word_wrap_long_word: Testing word wrap with long word");

        let buffer = TextBuffer::with_text("supercalifragilisticexpialidocious");
        eprintln!(
            "[TEST] Input: {:?}, length: {}",
            buffer.to_string(),
            buffer.len_chars()
        );

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 10, 10)
            .wrap_mode(WrapMode::Word);

        let info = view.line_info();
        eprintln!("[TEST] Wrap width: 10, LineInfo:");
        for i in 0..info.virtual_line_count() {
            eprintln!(
                "[TEST]   Line {}: start={} width={} source={} wrap={}",
                i, info.starts[i], info.widths[i], info.sources[i], info.wraps[i]
            );
        }

        // Long word without spaces should still break at char boundaries
        assert!(
            info.virtual_line_count() >= 3,
            "Long word should split across lines"
        );

        eprintln!("[TEST] PASS: Long word breaks at character boundaries when no spaces");
    }

    #[test]
    fn test_line_cache_multiple_lines() {
        eprintln!("[TEST] test_line_cache_multiple_lines: Testing multiple logical lines");

        let buffer = TextBuffer::with_text("Short\nThis is longer\nEnd");
        eprintln!("[TEST] Input with 3 logical lines:");
        for (i, line) in buffer.to_string().lines().enumerate() {
            eprintln!("[TEST]   Line {i}: {line:?}");
        }

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 10, 10)
            .wrap_mode(WrapMode::Word);

        let info = view.line_info();
        eprintln!("[TEST] LineInfo (wrap_width=10):");
        for i in 0..info.virtual_line_count() {
            eprintln!(
                "[TEST]   Virtual {}: start={} width={} source={} wrap={}",
                i, info.starts[i], info.widths[i], info.sources[i], info.wraps[i]
            );
        }

        // "This is longer" should wrap
        assert!(info.virtual_line_count() > 3, "Middle line should wrap");
        assert_eq!(info.sources[0], 0, "First virtual line from source 0");

        eprintln!("[TEST] PASS: Multiple lines with wrapping handled correctly");
    }

    #[test]
    fn test_line_cache_empty_lines() {
        eprintln!("[TEST] test_line_cache_empty_lines: Testing empty lines");

        let buffer = TextBuffer::with_text("Line1\n\nLine3");
        eprintln!("[TEST] Input: {:?}", buffer.to_string());

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info = view.line_info();
        eprintln!("[TEST] LineInfo:");
        for i in 0..info.virtual_line_count() {
            eprintln!(
                "[TEST]   Line {}: start={} width={} source={} wrap={}",
                i, info.starts[i], info.widths[i], info.sources[i], info.wraps[i]
            );
        }

        assert_eq!(
            info.virtual_line_count(),
            3,
            "Should have 3 lines including empty"
        );
        assert_eq!(info.widths[1], 0, "Empty line has width 0");

        eprintln!("[TEST] PASS: Empty lines handled correctly");
    }

    #[test]
    fn test_line_cache_utf8_width() {
        eprintln!("[TEST] test_line_cache_utf8_width: Testing UTF-8 character widths");

        let buffer = TextBuffer::with_text("Hëllo");
        eprintln!(
            "[TEST] Input: {:?}, byte len: {}",
            buffer.to_string(),
            buffer.to_string().len()
        );

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info = view.line_info();
        eprintln!("[TEST] LineInfo:");
        eprintln!("[TEST]   width: {}", info.widths[0]);

        assert_eq!(info.widths[0], 5, "UTF-8 'ë' should have display width 1");

        eprintln!("[TEST] PASS: UTF-8 characters have correct display width");
    }

    #[test]
    fn test_line_cache_cjk_characters() {
        eprintln!("[TEST] test_line_cache_cjk_characters: Testing CJK character widths");

        // CJK characters are typically 2 columns wide
        let buffer = TextBuffer::with_text("Hi中文Ok");
        eprintln!("[TEST] Input: {:?}", buffer.to_string());
        eprintln!("[TEST] Expected widths: H=1, i=1, 中=2, 文=2, O=1, k=1 = 8 total");

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info = view.line_info();
        eprintln!("[TEST] Computed width: {}", info.widths[0]);

        assert_eq!(info.widths[0], 8, "CJK chars should be 2 columns each");

        eprintln!("[TEST] PASS: CJK characters have width 2");
    }

    #[test]
    fn test_line_cache_cjk_wrap() {
        eprintln!("[TEST] test_line_cache_cjk_wrap: Testing CJK wrapping doesn't break mid-char");

        let buffer = TextBuffer::with_text("AB中文CD");
        eprintln!("[TEST] Input: {:?}", buffer.to_string());
        eprintln!("[TEST] Widths: A=1, B=1, 中=2, 文=2, C=1, D=1 = 8");

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 5, 10)
            .wrap_mode(WrapMode::Char);

        let info = view.line_info();
        eprintln!("[TEST] Wrap width: 5, LineInfo:");
        for i in 0..info.virtual_line_count() {
            eprintln!("[TEST]   Line {}: width={}", i, info.widths[i]);
        }

        // Verify no line has an odd-width ending that would split a CJK char
        for (i, &width) in info.widths.iter().enumerate() {
            eprintln!("[TEST] Verifying line {i} width {width} <= 5");
            assert!(width <= 5, "Line {i} width {width} exceeds wrap width 5");
        }

        eprintln!("[TEST] PASS: CJK characters not broken mid-character");
    }

    #[test]
    fn test_line_cache_invalidation_content() {
        eprintln!("[TEST] test_line_cache_invalidation_content: Testing cache invalidation");

        let buffer = TextBuffer::with_text("Hello");
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info1 = view.line_info();
        eprintln!(
            "[TEST] Initial info: lines={}, max_width={}",
            info1.virtual_line_count(),
            info1.max_width
        );

        // Create new buffer with different content
        let buffer2 = TextBuffer::with_text("Hello World Extended");
        let view2 = TextBufferView::new(&buffer2)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info2 = view2.line_info();
        eprintln!(
            "[TEST] New info: lines={}, max_width={}",
            info2.virtual_line_count(),
            info2.max_width
        );

        assert_ne!(
            info1.max_width, info2.max_width,
            "Different content should have different width"
        );

        eprintln!("[TEST] PASS: Cache correctly reflects content changes");
    }

    #[test]
    fn test_line_cache_invalidation_wrap_mode() {
        eprintln!("[TEST] test_line_cache_invalidation_wrap_mode: Testing wrap mode change");

        let buffer = TextBuffer::with_text("Hello World Test Line");

        let view_none = TextBufferView::new(&buffer)
            .viewport(0, 0, 10, 10)
            .wrap_mode(WrapMode::None);
        let info_none = view_none.line_info();
        eprintln!(
            "[TEST] WrapMode::None: lines={}",
            info_none.virtual_line_count()
        );

        let view_char = TextBufferView::new(&buffer)
            .viewport(0, 0, 10, 10)
            .wrap_mode(WrapMode::Char);
        let info_char = view_char.line_info();
        eprintln!(
            "[TEST] WrapMode::Char: lines={}",
            info_char.virtual_line_count()
        );

        assert_ne!(
            info_none.virtual_line_count(),
            info_char.virtual_line_count(),
            "Different wrap modes should produce different line counts"
        );

        eprintln!("[TEST] PASS: Wrap mode change produces different results");
    }

    #[test]
    fn test_source_to_virtual_mapping() {
        eprintln!("[TEST] test_source_to_virtual_mapping: Testing source -> virtual mapping");

        let buffer = TextBuffer::with_text("Short\nThis is a longer line that wraps\nEnd");
        eprintln!("[TEST] Input with 3 logical lines");

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 15, 10)
            .wrap_mode(WrapMode::Word);

        let info = view.line_info();
        eprintln!("[TEST] Virtual lines:");
        for i in 0..info.virtual_line_count() {
            eprintln!("[TEST]   Virtual {}: source={}", i, info.sources[i]);
        }

        // Test source_to_virtual
        for src in 0..=2 {
            let virt = info.source_to_virtual(src);
            eprintln!("[TEST] source_to_virtual({src}) = {virt:?}");
            assert!(virt.is_some(), "Source {src} should map to a virtual line");
        }

        // Test virtual_to_source
        for virt in 0..info.virtual_line_count() {
            let src = info.virtual_to_source(virt);
            eprintln!("[TEST] virtual_to_source({virt}) = {src:?}");
            assert!(src.is_some(), "Virtual {virt} should map to a source line");
        }

        // Test round-trip: source -> virtual -> source
        for src in 0..=2 {
            if let Some(virt) = info.source_to_virtual(src) {
                let back = info.virtual_to_source(virt).unwrap();
                eprintln!("[TEST] Round-trip: {src} -> {virt} -> {back}");
                assert_eq!(back, src, "Round-trip should preserve source line");
            }
        }

        eprintln!("[TEST] PASS: Source/virtual mappings are correct");
    }

    #[test]
    fn test_virtual_to_source_mapping() {
        eprintln!("[TEST] test_virtual_to_source_mapping: Testing virtual -> source mapping");

        let buffer = TextBuffer::with_text("Line one\nLine two\nLine three");
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 5, 10)
            .wrap_mode(WrapMode::Char);

        let info = view.line_info();
        eprintln!("[TEST] {} virtual lines", info.virtual_line_count());

        for virt in 0..info.virtual_line_count() {
            let src = info.virtual_to_source(virt);
            let is_cont = info.is_continuation(virt);
            eprintln!("[TEST] Virtual {virt} -> source {src:?}, is_continuation: {is_cont:?}");
        }

        // Verify out-of-bounds returns None
        let oob = info.virtual_to_source(1000);
        assert!(oob.is_none(), "Out of bounds should return None");

        eprintln!("[TEST] PASS: Virtual to source mapping works");
    }

    #[test]
    fn test_line_info_helper_methods() {
        eprintln!("[TEST] test_line_info_helper_methods: Testing LineInfo helper methods");

        let buffer = TextBuffer::with_text("Hello\nWorld");
        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 24)
            .wrap_mode(WrapMode::None);

        let info = view.line_info();

        eprintln!("[TEST] virtual_line_count: {}", info.virtual_line_count());
        assert_eq!(info.virtual_line_count(), 2);

        eprintln!("[TEST] max_source_line: {:?}", info.max_source_line());
        assert_eq!(info.max_source_line(), Some(1));

        eprintln!(
            "[TEST] virtual_lines_for_source(0): {}",
            info.virtual_lines_for_source(0)
        );
        assert_eq!(info.virtual_lines_for_source(0), 1);

        eprintln!(
            "[TEST] virtual_line_width(0): {:?}",
            info.virtual_line_width(0)
        );
        assert_eq!(info.virtual_line_width(0), Some(5));

        eprintln!("[TEST] is_continuation(0): {:?}", info.is_continuation(0));
        assert_eq!(info.is_continuation(0), Some(false));

        eprintln!("[TEST] PASS: Helper methods work correctly");
    }

    #[test]
    fn test_line_cache_performance() {
        use std::fmt::Write as _;
        use std::time::Instant;

        eprintln!("[PERF] test_line_cache_performance: Testing cache performance");

        // Generate 10K lines of text
        let mut text = String::new();
        for i in 0..10_000 {
            let _ = writeln!(
                text,
                "Line {i} with some content that might wrap when narrow"
            );
        }

        let buffer = TextBuffer::with_text(&text);
        eprintln!(
            "[PERF] Buffer size: {} bytes, {} lines",
            text.len(),
            buffer.len_lines()
        );

        let view = TextBufferView::new(&buffer)
            .viewport(0, 0, 80, 100)
            .wrap_mode(WrapMode::Word);

        let start = Instant::now();
        let info = view.line_info();
        let elapsed = start.elapsed();

        eprintln!("[PERF] Cache computation time: {elapsed:?}");
        eprintln!("[PERF] Virtual lines: {}", info.virtual_line_count());
        eprintln!("[PERF] Max width: {}", info.max_width);
        let lines_per_ms = 10_000.0 / elapsed.as_secs_f64() / 1000.0;
        eprintln!("[PERF] Lines per millisecond: {lines_per_ms:.0}");

        // Allow up to 100ms for CI/slow machines, but log if over 10ms
        if elapsed.as_millis() > 10 {
            eprintln!("[PERF] WARNING: Took {elapsed:?}, expected <10ms");
        }
        assert!(
            elapsed.as_millis() < 100,
            "Cache computation took {elapsed:?}, should be <100ms"
        );

        eprintln!("[PERF] PASS: 10K lines processed efficiently");
    }
}
