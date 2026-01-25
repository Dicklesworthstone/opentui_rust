//! Editor view with visual cursor and selection rendering.

// if-let-else is clearer than map_or for complex logic
#![allow(clippy::option_if_let_else)]

use crate::buffer::OptimizedBuffer;
use crate::color::Rgba;
use crate::highlight::theme::Theme;
use crate::highlight::tokenizer::TokenizerRegistry;
use crate::style::Style;
use crate::text::view::{LocalSelection, Selection, Viewport};
use crate::text::{EditBuffer, TextBufferView, WrapMode};

/// Cursor style for rendering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    /// Block cursor.
    #[default]
    Block,
    /// Underline cursor.
    Underline,
    /// Vertical bar cursor.
    Bar,
}

/// Visual cursor information in wrapped view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VisualCursor {
    pub visual_row: u32,
    pub visual_col: u32,
    pub logical_row: u32,
    pub logical_col: u32,
    pub offset: u32,
}

/// A virtual line segment for visual navigation with wrapped text.
#[derive(Clone, Debug)]
struct VirtualLine {
    source_line: usize,
    byte_start: usize,
    byte_end: usize,
    width: usize,
    is_wrap: bool,
}

/// Editor view wrapping an EditBuffer with visual rendering.
pub struct EditorView {
    edit_buffer: EditBuffer,
    cursor_style: Style,
    cursor_shape: CursorShape,
    selection_style: Style,
    wrap_mode: WrapMode,
    scroll_x: u32,
    scroll_y: u32,
    line_numbers: bool,
    line_number_style: Style,
    viewport: Option<Viewport>,
    scroll_margin: f32,
    selection_follow_cursor: bool,
    selection: Option<Selection>,
    local_selection: Option<LocalSelection>,
}

impl EditorView {
    /// Create a new editor view.
    #[must_use]
    pub fn new(edit_buffer: EditBuffer) -> Self {
        Self {
            edit_buffer,
            cursor_style: Style::builder().inverse().build(),
            cursor_shape: CursorShape::Block,
            selection_style: Style::builder().bg(Rgba::from_rgb_u8(60, 60, 120)).build(),
            wrap_mode: WrapMode::None,
            scroll_x: 0,
            scroll_y: 0,
            line_numbers: false,
            line_number_style: Style::dim(),
            viewport: None,
            scroll_margin: 0.1,
            selection_follow_cursor: false,
            selection: None,
            local_selection: None,
        }
    }

    /// Create an empty editor view.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(EditBuffer::new())
    }

    /// Get the edit buffer.
    #[must_use]
    pub fn edit_buffer(&self) -> &EditBuffer {
        &self.edit_buffer
    }

    /// Get mutable access to the edit buffer.
    pub fn edit_buffer_mut(&mut self) -> &mut EditBuffer {
        &mut self.edit_buffer
    }

    /// Set cursor style.
    pub fn set_cursor_style(&mut self, style: Style) {
        self.cursor_style = style;
    }

    /// Set cursor shape.
    pub fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.cursor_shape = shape;
    }

    /// Set selection style.
    pub fn set_selection_style(&mut self, style: Style) {
        self.selection_style = style;
    }

    /// Set wrap mode.
    pub fn set_wrap_mode(&mut self, mode: WrapMode) {
        self.wrap_mode = mode;
    }

    /// Set the viewport.
    pub fn set_viewport(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.viewport = Some(Viewport::new(x, y, width, height));
    }

    /// Set scroll margin (0.0-0.5 of viewport).
    pub fn set_scroll_margin(&mut self, margin: f32) {
        self.scroll_margin = margin.clamp(0.0, 0.5);
    }

    /// Enable or disable selection following the cursor.
    pub fn set_selection_follow_cursor(&mut self, enabled: bool) {
        self.selection_follow_cursor = enabled;
    }

    /// Enable or disable line numbers.
    pub fn set_line_numbers(&mut self, enabled: bool) {
        self.line_numbers = enabled;
    }

    /// Set line number style.
    pub fn set_line_number_style(&mut self, style: Style) {
        self.line_number_style = style;
    }

    /// Enable syntax highlighting using a tokenizer registry and file extension.
    pub fn enable_highlighting_for_extension(
        &mut self,
        registry: &TokenizerRegistry,
        extension: &str,
    ) -> bool {
        if let Some(tokenizer) = registry.for_extension_shared(extension) {
            self.edit_buffer
                .highlighted_buffer_mut()
                .set_tokenizer(Some(tokenizer));
            true
        } else {
            false
        }
    }

    /// Disable syntax highlighting.
    pub fn disable_highlighting(&mut self) {
        self.edit_buffer
            .highlighted_buffer_mut()
            .set_tokenizer(None);
    }

    /// Set the highlighting theme.
    pub fn set_highlighting_theme(&mut self, theme: Theme) {
        self.line_number_style = Style::fg(theme.line_number());
        self.edit_buffer.highlighted_buffer_mut().set_theme(theme);
    }

    /// Set selection range by character offsets.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        self.selection = Some(Selection::new(start, end, self.selection_style));
    }

    /// Clear selection range.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Delete selected text (offset-based selection).
    pub fn delete_selected_text(&mut self) {
        if let Some(sel) = self.selection.take() {
            self.edit_buffer
                .delete_range_offsets(sel.start.min(sel.end), sel.start.max(sel.end));
        }
    }

    /// Set a local (viewport) selection.
    pub fn set_local_selection(
        &mut self,
        anchor_x: u32,
        anchor_y: u32,
        focus_x: u32,
        focus_y: u32,
    ) {
        self.local_selection = Some(LocalSelection::new(
            anchor_x,
            anchor_y,
            focus_x,
            focus_y,
            self.selection_style,
        ));
    }

    /// Clear local selection.
    pub fn clear_local_selection(&mut self) {
        self.local_selection = None;
    }

    /// Start a new selection at current cursor position.
    pub fn start_selection(&mut self) {
        let offset = self.edit_buffer.cursor().offset;
        self.selection = Some(Selection::new(offset, offset, self.selection_style));
    }

    /// Extend selection to current cursor position.
    ///
    /// If no selection exists, starts a new selection at the cursor.
    pub fn extend_selection_to_cursor(&mut self) {
        if let Some(sel) = &mut self.selection {
            sel.end = self.edit_buffer.cursor().offset;
        } else {
            self.start_selection();
        }
    }

    /// Get the selected text, if any.
    ///
    /// Returns the text between selection start and end, regardless of direction.
    /// Returns `None` if there is no selection or if the selection is empty.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let (start, end) = (sel.start.min(sel.end), sel.start.max(sel.end));
        if start == end {
            return None; // Empty selection
        }
        Some(
            self.edit_buffer
                .buffer()
                .rope()
                .slice(start..end)
                .to_string(),
        )
    }

    /// Scroll to make cursor visible.
    pub fn scroll_to_cursor(&mut self, viewport_width: u32, viewport_height: u32) {
        let cursor = self.edit_buffer.cursor();
        let gutter_width = if self.line_numbers {
            self.gutter_width()
        } else {
            0
        };
        let text_width = viewport_width.saturating_sub(gutter_width);

        let margin_rows = (viewport_height as f32 * self.scroll_margin).ceil() as u32;
        let margin_cols = (text_width as f32 * self.scroll_margin).ceil() as u32;

        let (visual_row, visual_col) = if self.wrap_mode == WrapMode::None {
            (cursor.row as u32, cursor.col as u32)
        } else {
            let view = TextBufferView::new(self.edit_buffer.buffer())
                .viewport(0, 0, text_width, viewport_height)
                .wrap_mode(self.wrap_mode);
            view.visual_position_for_offset(cursor.offset)
        };

        // Vertical scrolling
        if visual_row < self.scroll_y + margin_rows {
            self.scroll_y = visual_row.saturating_sub(margin_rows);
        } else if visual_row >= self.scroll_y + viewport_height.saturating_sub(margin_rows) {
            self.scroll_y =
                visual_row.saturating_sub(viewport_height.saturating_sub(margin_rows + 1));
        }

        // Horizontal scrolling (if not wrapping)
        if self.wrap_mode == WrapMode::None {
            if visual_col < self.scroll_x + margin_cols {
                self.scroll_x = visual_col.saturating_sub(margin_cols);
            } else if visual_col >= self.scroll_x + text_width.saturating_sub(margin_cols) {
                self.scroll_x =
                    visual_col.saturating_sub(text_width.saturating_sub(margin_cols + 1));
            }
        } else {
            self.scroll_x = 0;
        }

        if self.selection_follow_cursor {
            if let Some(sel) = &mut self.selection {
                sel.end = cursor.offset;
            }
        }
    }

    /// Set scroll position.
    pub fn set_scroll(&mut self, x: u32, y: u32) {
        self.scroll_x = x;
        self.scroll_y = y;
    }

    /// Get scroll position.
    #[must_use]
    pub fn scroll(&self) -> (u32, u32) {
        (self.scroll_x, self.scroll_y)
    }

    /// Move cursor up one visual line (accounts for wrapping).
    ///
    /// In wrapped mode, this moves up within a wrapped line segment.
    /// In non-wrapped mode, this is equivalent to move_up().
    pub fn move_up_visual(&mut self, viewport_width: u32, viewport_height: u32) {
        if self.wrap_mode == WrapMode::None {
            self.edit_buffer.move_up();
            return;
        }

        let gutter_width = if self.line_numbers {
            self.gutter_width()
        } else {
            0
        };
        let text_width = viewport_width.saturating_sub(gutter_width);
        let vlines = self.build_virtual_lines(text_width, viewport_height);

        let cursor = self.edit_buffer.cursor();
        let byte_offset = self.edit_buffer.buffer().rope().char_to_byte(cursor.offset);

        // Find current visual line (handles cursor at newline positions)
        let current_vline_idx = Self::find_vline_index(&vlines, byte_offset);

        if current_vline_idx == 0 {
            return; // Already at top
        }

        // Calculate visual column within current visual line
        let current_vline = &vlines[current_vline_idx];
        let visual_col = self.visual_col_in_vline(current_vline, cursor.offset);

        let prev_vline = &vlines[current_vline_idx - 1];
        let target_offset = self.offset_at_visual_col(prev_vline, visual_col, text_width);
        self.edit_buffer.set_cursor_by_offset(target_offset);
    }

    /// Move cursor down one visual line (accounts for wrapping).
    ///
    /// In wrapped mode, this moves down within a wrapped line segment.
    /// In non-wrapped mode, this is equivalent to move_down().
    pub fn move_down_visual(&mut self, viewport_width: u32, viewport_height: u32) {
        if self.wrap_mode == WrapMode::None {
            self.edit_buffer.move_down();
            return;
        }

        let gutter_width = if self.line_numbers {
            self.gutter_width()
        } else {
            0
        };
        let text_width = viewport_width.saturating_sub(gutter_width);
        let vlines = self.build_virtual_lines(text_width, viewport_height);

        let cursor = self.edit_buffer.cursor();
        let byte_offset = self.edit_buffer.buffer().rope().char_to_byte(cursor.offset);

        // Find current visual line (handles cursor at newline positions)
        let current_vline_idx = Self::find_vline_index(&vlines, byte_offset);

        if current_vline_idx + 1 >= vlines.len() {
            return; // Already at bottom
        }

        // Calculate visual column within current visual line
        let current_vline = &vlines[current_vline_idx];
        let visual_col = self.visual_col_in_vline(current_vline, cursor.offset);

        let next_vline = &vlines[current_vline_idx + 1];
        let target_offset = self.offset_at_visual_col(next_vline, visual_col, text_width);
        self.edit_buffer.set_cursor_by_offset(target_offset);
    }

    /// Get the start of the current visual line.
    ///
    /// In wrapped mode, returns the start of the current wrapped segment.
    /// In non-wrapped mode, returns the start of the logical line.
    #[must_use]
    pub fn get_visual_sol(&self, viewport_width: u32, viewport_height: u32) -> usize {
        if self.wrap_mode == WrapMode::None {
            let cursor = self.edit_buffer.cursor();
            return self.edit_buffer.buffer().rope().line_to_char(cursor.row);
        }

        let gutter_width = if self.line_numbers {
            self.gutter_width()
        } else {
            0
        };
        let text_width = viewport_width.saturating_sub(gutter_width);
        let vlines = self.build_virtual_lines(text_width, viewport_height);

        let cursor = self.edit_buffer.cursor();
        let byte_offset = self.edit_buffer.buffer().rope().char_to_byte(cursor.offset);

        let idx = Self::find_vline_index(&vlines, byte_offset);
        if idx < vlines.len() {
            return self
                .edit_buffer
                .buffer()
                .rope()
                .byte_to_char(vlines[idx].byte_start);
        }

        cursor.offset
    }

    /// Get the end of the current visual line.
    ///
    /// In wrapped mode, returns the end of the current wrapped segment.
    /// In non-wrapped mode, returns the end of the logical line (before newline).
    #[must_use]
    pub fn get_visual_eol(&self, viewport_width: u32, viewport_height: u32) -> usize {
        if self.wrap_mode == WrapMode::None {
            return self.edit_buffer.get_eol();
        }

        let gutter_width = if self.line_numbers {
            self.gutter_width()
        } else {
            0
        };
        let text_width = viewport_width.saturating_sub(gutter_width);
        let vlines = self.build_virtual_lines(text_width, viewport_height);

        let cursor = self.edit_buffer.cursor();
        let byte_offset = self.edit_buffer.buffer().rope().char_to_byte(cursor.offset);

        let idx = Self::find_vline_index(&vlines, byte_offset);
        if idx < vlines.len() {
            return self
                .edit_buffer
                .buffer()
                .rope()
                .byte_to_char(vlines[idx].byte_end);
        }

        cursor.offset
    }

    /// Move cursor to start of visual line.
    pub fn move_to_visual_sol(&mut self, viewport_width: u32, viewport_height: u32) {
        let sol = self.get_visual_sol(viewport_width, viewport_height);
        self.edit_buffer.set_cursor_by_offset(sol);
    }

    /// Move cursor to end of visual line.
    pub fn move_to_visual_eol(&mut self, viewport_width: u32, viewport_height: u32) {
        let eol = self.get_visual_eol(viewport_width, viewport_height);
        self.edit_buffer.set_cursor_by_offset(eol);
    }

    /// Get visual cursor info for a given viewport size.
    #[must_use]
    pub fn visual_cursor(&self, viewport_width: u32, viewport_height: u32) -> VisualCursor {
        let cursor = self.edit_buffer.cursor();
        let gutter_width = if self.line_numbers {
            self.gutter_width()
        } else {
            0
        };
        let text_width = viewport_width.saturating_sub(gutter_width);
        let view = TextBufferView::new(self.edit_buffer.buffer())
            .viewport(0, 0, text_width, viewport_height)
            .wrap_mode(self.wrap_mode);
        let (visual_row, visual_col) = if self.wrap_mode == WrapMode::None {
            (cursor.row as u32, cursor.col as u32)
        } else {
            view.visual_position_for_offset(cursor.offset)
        };

        VisualCursor {
            visual_row,
            visual_col,
            logical_row: cursor.row as u32,
            logical_col: cursor.col as u32,
            offset: cursor.offset as u32,
        }
    }

    /// Calculate gutter width for line numbers.
    #[must_use]
    pub fn gutter_width(&self) -> u32 {
        if !self.line_numbers {
            return 0;
        }

        let line_count = self.edit_buffer.buffer().len_lines().max(1);
        let digits = line_count.ilog10() + 1;
        digits + 2 // digit count + padding
    }

    /// Build virtual line information for visual navigation.
    #[allow(clippy::too_many_lines)]
    fn build_virtual_lines(&self, text_width: u32, _viewport_height: u32) -> Vec<VirtualLine> {
        use unicode_segmentation::UnicodeSegmentation;

        let mut lines = Vec::new();
        let rope = self.edit_buffer.buffer().rope();
        let method = self.edit_buffer.buffer().width_method();
        let tab_width = self.edit_buffer.buffer().tab_width().max(1) as usize;
        let wrap_width = if self.wrap_mode != WrapMode::None && text_width > 0 {
            Some(text_width as usize)
        } else {
            None
        };

        for line_idx in 0..self.edit_buffer.buffer().len_lines() {
            let Some(line) = self.edit_buffer.buffer().line(line_idx) else {
                continue;
            };
            let line = line.trim_end_matches('\n').trim_end_matches('\r');

            let line_start_char = rope.line_to_char(line_idx);
            let line_start_byte = rope.char_to_byte(line_start_char);

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
                let width = crate::unicode::display_width_with_method(line, method);
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
            let mut last_break: Option<(usize, usize, usize)> = None;
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
                    crate::unicode::display_width_with_method(grapheme, method)
                };

                let is_ws = grapheme.chars().all(char::is_whitespace);
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
                            if g.chars().all(char::is_whitespace) {
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

    /// Find the virtual line index for a byte offset, handling cursor at newline positions.
    fn find_vline_index(vlines: &[VirtualLine], byte_offset: usize) -> usize {
        for (idx, vline) in vlines.iter().enumerate() {
            let is_last = idx == vlines.len() - 1;
            if byte_offset < vline.byte_start {
                continue;
            }
            // Cursor is within this line
            if byte_offset < vline.byte_end {
                return idx;
            }
            // Cursor is at byte_end (e.g., at newline position)
            if byte_offset == vline.byte_end {
                if is_last {
                    return idx;
                }
                // Check if next line is a different source line (new logical line)
                let next_vline = &vlines[idx + 1];
                if next_vline.source_line != vline.source_line {
                    return idx;
                }
                // Next line is wrap continuation, continue searching
            }
        }
        // Fallback to last line
        vlines.len().saturating_sub(1)
    }

    /// Find the character offset at a target visual column within a virtual line.
    fn offset_at_visual_col(
        &self,
        vline: &VirtualLine,
        target_col: usize,
        _text_width: u32,
    ) -> usize {
        use unicode_segmentation::UnicodeSegmentation;

        let rope = self.edit_buffer.buffer().rope();
        let char_start = rope.byte_to_char(vline.byte_start);
        let char_end = rope.byte_to_char(vline.byte_end);
        let line = rope.slice(char_start..char_end).to_string();

        let method = self.edit_buffer.buffer().width_method();
        let tab_width = self.edit_buffer.buffer().tab_width().max(1) as usize;

        let mut current_col = 0usize;
        let mut char_offset = char_start;

        for grapheme in line.graphemes(true) {
            if current_col >= target_col {
                break;
            }

            let g_width = if grapheme == "\t" {
                let offset = current_col % tab_width;
                tab_width - offset
            } else {
                crate::unicode::display_width_with_method(grapheme, method)
            };

            current_col += g_width;
            char_offset += grapheme.chars().count();
        }

        char_offset.min(char_end)
    }

    /// Calculate the visual column of a character offset within a virtual line.
    fn visual_col_in_vline(&self, vline: &VirtualLine, char_offset: usize) -> usize {
        use unicode_segmentation::UnicodeSegmentation;

        let rope = self.edit_buffer.buffer().rope();
        let char_start = rope.byte_to_char(vline.byte_start);
        let char_end = rope.byte_to_char(vline.byte_end).min(char_offset);
        let line = rope.slice(char_start..char_end).to_string();

        let method = self.edit_buffer.buffer().width_method();
        let tab_width = self.edit_buffer.buffer().tab_width().max(1) as usize;

        let mut width = 0usize;
        for grapheme in line.graphemes(true) {
            if grapheme == "\t" {
                let offset = width % tab_width;
                width += tab_width - offset;
            } else {
                width += crate::unicode::display_width_with_method(grapheme, method);
            }
        }

        width
    }

    /// Render to output buffer.
    pub fn render_to(
        &mut self,
        output: &mut OptimizedBuffer,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        self.edit_buffer
            .highlighted_buffer_mut()
            .update_highlighting();
        let (x, y, width, height) = if let Some(viewport) = self.viewport {
            (viewport.x, viewport.y, viewport.width, viewport.height)
        } else {
            (x, y, width, height)
        };

        let gutter_width = self.gutter_width();
        let text_x = x + gutter_width;
        let text_width = width.saturating_sub(gutter_width);

        // Render line numbers if enabled
        if self.line_numbers {
            self.render_line_numbers(output, x, y, gutter_width, height);
        }

        // Create a view and render text
        let mut view = TextBufferView::new(self.edit_buffer.buffer())
            .viewport(0, 0, text_width, height)
            .wrap_mode(self.wrap_mode)
            .scroll(self.scroll_x, self.scroll_y);

        if let Some(sel) = self.selection {
            view.set_selection(sel.start, sel.end, sel.style);
        }
        if let Some(local) = self.local_selection {
            view.set_local_selection(
                local.anchor_x,
                local.anchor_y,
                local.focus_x,
                local.focus_y,
                local.style,
            );
        }

        view.render_to(output, text_x as i32, y as i32);

        // Render cursor
        self.render_cursor(output, &view, text_x, y, text_width, height);
    }

    fn render_line_numbers(
        &self,
        output: &mut OptimizedBuffer,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        let start_line = self.scroll_y as usize;
        let end_line = (start_line + height as usize).min(self.edit_buffer.buffer().len_lines());
        let cursor_row = self.edit_buffer.cursor().row;

        for (offset, line_num) in (start_line..end_line).enumerate() {
            let display_num = line_num + 1;
            let s = format!("{display_num:>width$} ", width = (width - 1) as usize);

            let style = if line_num == cursor_row {
                self.line_number_style.with_bold()
            } else {
                self.line_number_style
            };

            output.draw_text(x, y + offset as u32, &s, style);
        }
    }

    fn render_cursor(
        &self,
        output: &mut OptimizedBuffer,
        view: &TextBufferView<'_>,
        text_x: u32,
        text_y: u32,
        _width: u32,
        _height: u32,
    ) {
        let cursor = self.edit_buffer.cursor();
        let (visual_row, visual_col) = if self.wrap_mode == WrapMode::None {
            (cursor.row as u32, cursor.col as u32)
        } else {
            view.visual_position_for_offset(cursor.offset)
        };

        if visual_row < self.scroll_y {
            return;
        }

        let visible_row = visual_row - self.scroll_y;
        let visible_col = if self.wrap_mode == WrapMode::None {
            visual_col.saturating_sub(self.scroll_x)
        } else {
            visual_col
        };

        let cursor_x = text_x + visible_col;
        let cursor_y = text_y + visible_row;

        if let Some(cell) = output.get_mut(cursor_x, cursor_y) {
            cell.apply_style(self.cursor_style);
        }
    }
}

impl Default for EditorView {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_view_basic() {
        let edit = EditBuffer::with_text("Hello\nWorld");
        let view = EditorView::new(edit);
        assert_eq!(view.edit_buffer().text(), "Hello\nWorld");
    }

    #[test]
    fn test_editor_scroll_to_cursor() {
        let mut edit = EditBuffer::with_text("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");
        edit.move_to(4, 0);
        let mut view = EditorView::new(edit);

        view.scroll_to_cursor(80, 3);
        assert!(view.scroll_y >= 2);
    }

    #[test]
    fn test_gutter_width() {
        let edit = EditBuffer::with_text(&"x\n".repeat(100));
        let mut view = EditorView::new(edit);
        view.set_line_numbers(true);

        // 100 lines = 3 digits + 2 padding = 5
        assert_eq!(view.gutter_width(), 5);
    }

    // =========================================================================
    // Visual Navigation Tests with Detailed Logging (bd-1tl requirements)
    // =========================================================================

    #[test]
    fn test_visual_move_up_no_wrap() {
        eprintln!("[TEST] test_visual_move_up_no_wrap");
        let text = "Line 1\nLine 2\nLine 3";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(1, 3); // Middle of line 2
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::None);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] WrapMode: None");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(80, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );
        assert_eq!(cursor.row, 1);

        view.move_up_visual(80, 24);

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(80, 24);
        eprintln!("[TEST] After move_up_visual:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        assert_eq!(cursor.row, 0, "Should move to line 0");
        eprintln!("[TEST] PASS: move_up_visual works without wrapping");
    }

    #[test]
    fn test_visual_move_up_with_wrap() {
        eprintln!("[TEST] test_visual_move_up_with_wrap");
        // Create text where second line wraps at width 10
        let text = "Short\nabcdefghij12345\nEnd";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(1, 12); // In the wrapped portion of line 1
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 10");
        eprintln!("[TEST] WrapMode: Char");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        let initial_visual_row = visual.visual_row;

        view.move_up_visual(10, 24);

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] After move_up_visual:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        assert!(
            visual.visual_row < initial_visual_row,
            "Visual row should decrease"
        );
        eprintln!("[TEST] PASS: move_up_visual works with wrapping");
    }

    #[test]
    fn test_visual_move_up_within_wrapped_line() {
        eprintln!("[TEST] test_visual_move_up_within_wrapped_line");
        // Single line that wraps multiple times
        let text = "abcdefghijklmnopqrstuvwxyz";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 15); // In the middle, past first wrap
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Text length: {} chars", text.len());
        eprintln!("[TEST] Viewport width: 10");
        eprintln!("[TEST] WrapMode: Char");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        // Should be on visual line 1 (chars 10-19)
        assert_eq!(
            visual.visual_row, 1,
            "Should start on visual line 1 (second wrap segment)"
        );

        view.move_up_visual(10, 24);

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] After move_up_visual:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        // Still on logical line 0, but visual line 0
        assert_eq!(cursor.row, 0, "Should stay on logical line 0");
        assert_eq!(visual.visual_row, 0, "Should move to visual line 0");
        eprintln!("[TEST] PASS: move_up_visual works within wrapped line");
    }

    #[test]
    fn test_visual_move_down_no_wrap() {
        eprintln!("[TEST] test_visual_move_down_no_wrap");
        let text = "Line 1\nLine 2\nLine 3";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 3);
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::None);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] WrapMode: None");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(80, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        view.move_down_visual(80, 24);

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(80, 24);
        eprintln!("[TEST] After move_down_visual:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        assert_eq!(cursor.row, 1, "Should move to line 1");
        eprintln!("[TEST] PASS: move_down_visual works without wrapping");
    }

    #[test]
    fn test_visual_move_down_with_wrap() {
        eprintln!("[TEST] test_visual_move_down_with_wrap");
        let text = "abcdefghij12345\nEnd";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 5); // In first wrap segment
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 10");
        eprintln!("[TEST] WrapMode: Char");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );
        assert_eq!(visual.visual_row, 0);

        view.move_down_visual(10, 24);

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] After move_down_visual:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        // Should move to visual line 1 (still logical line 0, wrapped portion)
        assert_eq!(visual.visual_row, 1, "Should move to visual line 1");
        assert_eq!(cursor.row, 0, "Should still be on logical line 0");
        eprintln!("[TEST] PASS: move_down_visual works with wrapping");
    }

    #[test]
    fn test_visual_move_down_within_wrapped_line() {
        eprintln!("[TEST] test_visual_move_down_within_wrapped_line");
        let text = "abcdefghijklmnopqrstuvwxyz";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 5); // In the first visual line
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Text length: {} chars", text.len());
        eprintln!("[TEST] Viewport width: 10");
        eprintln!("[TEST] WrapMode: Char");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );
        assert_eq!(visual.visual_row, 0);

        view.move_down_visual(10, 24);

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] After move_down_visual:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        assert_eq!(cursor.row, 0, "Should stay on logical line 0");
        assert_eq!(
            visual.visual_row, 1,
            "Should move to visual line 1 (second wrap segment)"
        );
        eprintln!("[TEST] PASS: move_down_visual works within wrapped line");
    }

    #[test]
    fn test_visual_line_start() {
        eprintln!("[TEST] test_visual_line_start");
        let text = "abcdefghij12345";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 12); // In the wrapped portion
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 10");
        eprintln!("[TEST] WrapMode: Char");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        let sol = view.get_visual_sol(10, 24);
        eprintln!("[TEST] Visual SOL offset: {sol}");
        eprintln!(
            "[TEST] Character at SOL: {:?}",
            text.chars().nth(sol).unwrap_or(' ')
        );

        // Cursor is at offset 12, which is in the second visual line (chars 10-14)
        // SOL should be 10 (start of second visual line)
        assert_eq!(
            sol, 10,
            "Visual line start should be 10 (start of wrap segment)"
        );
        eprintln!("[TEST] PASS: get_visual_sol returns correct offset");
    }

    #[test]
    fn test_visual_line_end() {
        eprintln!("[TEST] test_visual_line_end");
        let text = "abcdefghij12345";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 5); // In the first visual line
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 10");
        eprintln!("[TEST] WrapMode: Char");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!("[TEST] Initial position:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        let eol = view.get_visual_eol(10, 24);
        eprintln!("[TEST] Visual EOL offset: {eol}");
        eprintln!(
            "[TEST] Character before EOL: {:?}",
            text.chars().nth(eol.saturating_sub(1)).unwrap_or(' ')
        );

        // First visual line covers chars 0-9, so EOL should be 10
        assert_eq!(
            eol, 10,
            "Visual line end should be 10 (end of first wrap segment)"
        );
        eprintln!("[TEST] PASS: get_visual_eol returns correct offset");
    }

    #[test]
    fn test_visual_nav_preserves_column() {
        eprintln!("[TEST] test_visual_nav_preserves_column");
        let text = "Short\nMedium line\nAnother short";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(1, 8); // Column 8 in "Medium line"
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::None);

        eprintln!("[TEST] Text:");
        for (i, line) in text.lines().enumerate() {
            eprintln!("[TEST]   Line {}: {:?} (len={})", i, line, line.len());
        }

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] Initial: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        assert_eq!(cursor.col, 8);

        // Move up - "Short" only has 5 chars, so col should clamp
        view.move_up_visual(80, 24);

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] After up: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        assert_eq!(cursor.row, 0);
        // In non-wrapped mode, move_up uses edit_buffer.move_up()
        // which preserves column as much as possible

        // Move down twice
        view.move_down_visual(80, 24);
        view.move_down_visual(80, 24);

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] After 2x down: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        assert_eq!(cursor.row, 2);

        eprintln!("[TEST] PASS: Column position handled correctly during navigation");
    }

    #[test]
    fn test_visual_nav_at_buffer_start() {
        eprintln!("[TEST] test_visual_nav_at_buffer_start");
        let text = "Line 1\nLine 2";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 0);
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::None);

        eprintln!("[TEST] Text: {text:?}");

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] Initial: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );

        // At start, try to move up
        eprintln!("[TEST] At buffer start, calling move_up_visual");
        view.move_up_visual(80, 24);

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] After up at start: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        assert_eq!(cursor.row, 0, "Should stay at start row");
        assert_eq!(cursor.col, 0, "Should stay at start col");

        eprintln!("[TEST] PASS: Boundary condition at start handled (no crash)");
    }

    #[test]
    fn test_visual_nav_at_buffer_end() {
        eprintln!("[TEST] test_visual_nav_at_buffer_end");
        let text = "Line 1\nLine 2";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(1, 6);
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::None);

        eprintln!("[TEST] Text: {text:?}");

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] Initial: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );

        // At end, try to move down
        eprintln!("[TEST] At buffer end, calling move_down_visual");
        view.move_down_visual(80, 24);

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] After down at end: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        assert_eq!(cursor.row, 1, "Should stay at end row");

        eprintln!("[TEST] PASS: Boundary condition at end handled (no crash)");
    }

    #[test]
    fn test_visual_nav_wide_characters() {
        eprintln!("[TEST] test_visual_nav_wide_characters");
        // CJK characters are 2 columns wide
        let text = "ABC\u{4e2d}\u{6587}DEF"; // "ABC中文DEF" - 3 + 4 + 3 = 10 display cols
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 0);
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Text char count: {}", text.chars().count());
        eprintln!("[TEST] Expected widths:");
        eprintln!("[TEST]   'ABC' = 3 cols");
        eprintln!("[TEST]   '中文' = 4 cols (2 chars x 2)");
        eprintln!("[TEST]   'DEF' = 3 cols");
        eprintln!("[TEST]   Total = 10 cols");
        eprintln!("[TEST] Viewport width: 8");

        let visual = view.visual_cursor(8, 24);
        eprintln!(
            "[TEST] At offset 0: visual_row={} visual_col={}",
            visual.visual_row, visual.visual_col
        );

        // Move through the text
        view.edit_buffer_mut().move_to(0, 5); // After "ABC中文"
        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(8, 24);
        eprintln!("[TEST] At middle:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        // Navigate and verify no crash
        view.move_up_visual(8, 24);
        let cursor = view.edit_buffer().cursor();
        eprintln!("[TEST] After up: char offset={}", cursor.offset);

        // Verify we're at a valid char index (cursor.offset is a char offset, not byte)
        // The cursor offset should be within valid char range
        let char_count = text.chars().count();
        assert!(
            cursor.offset <= char_count,
            "Cursor char offset should be within valid range (got {} for text with {} chars)",
            cursor.offset,
            char_count
        );

        // Convert to byte offset to verify byte boundary
        let byte_offset: usize = text.chars().take(cursor.offset).map(char::len_utf8).sum();
        assert!(
            text.is_char_boundary(byte_offset),
            "Byte offset {byte_offset} should be at valid char boundary"
        );
        eprintln!(
            "[TEST] Verified: char offset={} -> byte offset={} is valid",
            cursor.offset, byte_offset
        );

        eprintln!("[TEST] PASS: Wide character navigation works");
    }

    #[test]
    fn test_visual_nav_emoji_grapheme_clusters() {
        eprintln!("[TEST] test_visual_nav_emoji_grapheme_clusters");
        // Family emoji (ZWJ sequence) is multiple codepoints but single grapheme cluster
        // The cursor should move across the entire emoji as one unit
        let text = "AB👨\u{200D}👩\u{200D}👧CD"; // "AB" + family emoji + "CD"
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 0);
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: 'AB' + family emoji + 'CD'");
        eprintln!("[TEST] Display widths: A=1, B=1, family=2, C=1, D=1 = 6 total");
        eprintln!("[TEST] Viewport width: 10");

        let visual = view.visual_cursor(10, 24);
        eprintln!(
            "[TEST] At offset 0: visual_row={} visual_col={}",
            visual.visual_row, visual.visual_col
        );
        assert_eq!(visual.visual_col, 0, "Start at column 0");

        // Move right twice to get past "AB"
        view.edit_buffer_mut().move_right();
        view.edit_buffer_mut().move_right();
        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!(
            "[TEST] After 2 moves: offset={}, visual_col={}",
            cursor.offset, visual.visual_col
        );
        assert_eq!(visual.visual_col, 2, "After 'AB', visual col should be 2");

        // Move right once - should skip entire emoji grapheme cluster
        view.edit_buffer_mut().move_right();
        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!(
            "[TEST] After emoji: offset={}, visual_col={}",
            cursor.offset, visual.visual_col
        );
        // Visual column should be 4 (A=1 + B=1 + emoji=2)
        assert_eq!(
            visual.visual_col, 4,
            "After emoji, visual col should be 4 (emoji width is 2)"
        );

        // Verify cursor is at 'C' - the offset should account for all emoji codepoints
        let char_count = text.chars().count();
        assert!(
            cursor.offset <= char_count,
            "Cursor offset {} should be within text length {}",
            cursor.offset,
            char_count
        );

        eprintln!("[TEST] PASS: Emoji grapheme cluster navigation works");
    }

    #[test]
    fn test_visual_word_wrap_mode() {
        eprintln!("[TEST] test_visual_word_wrap_mode");
        let text = "Hello world test";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 0);
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Word);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 8");
        eprintln!("[TEST] WrapMode: Word");

        // With width 8 and word wrap:
        // "Hello " (6) fits
        // "world " (6) fits on next line
        // "test" (4) fits on next line

        let visual = view.visual_cursor(8, 24);
        eprintln!(
            "[TEST] At start: visual_row={} visual_col={}",
            visual.visual_row, visual.visual_col
        );

        view.move_down_visual(8, 24);

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(8, 24);
        eprintln!("[TEST] After move_down_visual:");
        eprintln!(
            "[TEST]   Logical: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );
        eprintln!(
            "[TEST]   Visual: row={} col={}",
            visual.visual_row, visual.visual_col
        );

        // Should have moved to a new visual line
        assert!(visual.visual_row > 0 || cursor.offset > 0, "Should move");
        eprintln!("[TEST] PASS: Word wrap mode navigation works");
    }

    #[test]
    fn test_move_to_visual_sol_wrapped() {
        eprintln!("[TEST] test_move_to_visual_sol_wrapped");
        let text = "abcdefghij12345";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 12); // In the wrapped portion
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 10");

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] Initial: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );

        view.move_to_visual_sol(10, 24);

        let cursor = view.edit_buffer().cursor();
        eprintln!("[TEST] After move_to_visual_sol: offset={}", cursor.offset);

        // Should be at start of second visual line
        assert_eq!(cursor.offset, 10, "Should move to visual line start");
        eprintln!("[TEST] PASS: move_to_visual_sol works with wrapping");
    }

    #[test]
    fn test_move_to_visual_eol_wrapped() {
        eprintln!("[TEST] test_move_to_visual_eol_wrapped");
        let text = "abcdefghij12345";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 5); // In the first visual line
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 10");

        let cursor = view.edit_buffer().cursor();
        eprintln!(
            "[TEST] Initial: row={} col={} offset={}",
            cursor.row, cursor.col, cursor.offset
        );

        view.move_to_visual_eol(10, 24);

        let cursor = view.edit_buffer().cursor();
        eprintln!("[TEST] After move_to_visual_eol: offset={}", cursor.offset);

        // Should be at end of first visual line
        assert_eq!(cursor.offset, 10, "Should move to visual line end");
        eprintln!("[TEST] PASS: move_to_visual_eol works with wrapping");
    }

    #[test]
    fn test_visual_cursor_info() {
        eprintln!("[TEST] test_visual_cursor_info");
        let text = "abcdefghij12345";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(0, 12);
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text: {text:?}");
        eprintln!("[TEST] Viewport width: 10");

        let vc = view.visual_cursor(10, 24);
        eprintln!("[TEST] VisualCursor:");
        eprintln!("[TEST]   visual_row: {}", vc.visual_row);
        eprintln!("[TEST]   visual_col: {}", vc.visual_col);
        eprintln!("[TEST]   logical_row: {}", vc.logical_row);
        eprintln!("[TEST]   logical_col: {}", vc.logical_col);
        eprintln!("[TEST]   offset: {}", vc.offset);

        // At offset 12, should be on visual row 1, col 2
        assert_eq!(vc.logical_row, 0, "Logical row should be 0");
        assert_eq!(vc.logical_col, 12, "Logical col should be 12");
        assert_eq!(vc.visual_row, 1, "Visual row should be 1 (second wrap)");
        assert_eq!(vc.visual_col, 2, "Visual col should be 2 (12 - 10)");

        eprintln!("[TEST] PASS: visual_cursor returns correct info");
    }

    #[test]
    fn test_visual_navigation_multiline_wrapped() {
        eprintln!("[TEST] test_visual_navigation_multiline_wrapped");
        let text = "Short\nabcdefghij12345\nEnd";
        let mut edit = EditBuffer::with_text(text);
        edit.move_to(1, 0); // Start of second line
        let mut view = EditorView::new(edit);
        view.set_wrap_mode(WrapMode::Char);

        eprintln!("[TEST] Text lines:");
        for (i, line) in text.lines().enumerate() {
            eprintln!("[TEST]   Line {}: {:?} (len={})", i, line, line.len());
        }
        eprintln!("[TEST] Viewport width: 10");
        eprintln!("[TEST] Visual layout:");
        eprintln!("[TEST]   Visual 0: 'Short' (source line 0)");
        eprintln!("[TEST]   Visual 1: 'abcdefghij' (source line 1, wrap 1)");
        eprintln!("[TEST]   Visual 2: '12345' (source line 1, wrap 2)");
        eprintln!("[TEST]   Visual 3: 'End' (source line 2)");

        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!(
            "[TEST] Initial: logical=({},{}) visual=({},{})",
            cursor.row, cursor.col, visual.visual_row, visual.visual_col
        );

        let initial_visual_row = visual.visual_row;

        // Move down through the wrapped line - should advance visually
        view.move_down_visual(10, 24);
        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!(
            "[TEST] After 1 down: logical=({},{}) visual=({},{})",
            cursor.row, cursor.col, visual.visual_row, visual.visual_col
        );

        // Should still be on logical line 1 (in wrapped portion)
        assert_eq!(cursor.row, 1, "Still on logical line 1");

        // Move down again
        view.move_down_visual(10, 24);
        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!(
            "[TEST] After 2 down: logical=({},{}) visual=({},{})",
            cursor.row, cursor.col, visual.visual_row, visual.visual_col
        );

        // Move down a third time to ensure we can reach the last line
        view.move_down_visual(10, 24);
        let cursor = view.edit_buffer().cursor();
        let visual = view.visual_cursor(10, 24);
        eprintln!(
            "[TEST] After 3 down: logical=({},{}) visual=({},{})",
            cursor.row, cursor.col, visual.visual_row, visual.visual_col
        );

        // After enough downs, we should eventually reach line 2 or be at end of buffer
        // The key test: visual navigation should progressively move through the content
        assert!(
            visual.visual_row > initial_visual_row,
            "Visual row should have increased from {} to {}",
            initial_visual_row,
            visual.visual_row
        );

        eprintln!("[TEST] PASS: Multiline wrapped navigation progresses through content");
    }

    // =========================================================================
    // Selection Extension Tests (bd-rqd requirements)
    // =========================================================================

    #[test]
    fn test_start_selection() {
        eprintln!("[TEST] test_start_selection");
        let buffer = EditBuffer::with_text("Hello, World!");
        let mut view = EditorView::new(buffer);

        view.edit_buffer_mut().set_cursor_by_offset(7); // After ", "
        eprintln!("[TEST] Cursor at offset 7");

        view.start_selection();

        assert!(view.selection.is_some());
        let sel = view.selection.unwrap();
        eprintln!("[TEST] Selection: start={} end={}", sel.start, sel.end);

        assert_eq!(sel.start, 7);
        assert_eq!(sel.end, 7, "New selection should have same start and end");

        eprintln!("[TEST] PASS: start_selection creates selection at cursor");
    }

    #[test]
    fn test_extend_selection_to_cursor() {
        eprintln!("[TEST] test_extend_selection_to_cursor");
        let buffer = EditBuffer::with_text("Hello, World!");
        let mut view = EditorView::new(buffer);

        // Start selection at position 0
        view.start_selection();
        eprintln!("[TEST] Started selection at 0");

        // Move cursor and extend
        view.edit_buffer_mut().set_cursor_by_offset(5); // After "Hello"
        view.extend_selection_to_cursor();

        let sel = view.selection.unwrap();
        eprintln!(
            "[TEST] After extending: start={} end={}",
            sel.start, sel.end
        );

        assert_eq!(sel.start, 0);
        assert_eq!(sel.end, 5, "Selection should extend to cursor");

        // Extend further
        view.edit_buffer_mut().set_cursor_by_offset(13); // End
        view.extend_selection_to_cursor();

        let sel = view.selection.unwrap();
        eprintln!(
            "[TEST] Extended to end: start={} end={}",
            sel.start, sel.end
        );
        assert_eq!(sel.end, 13);

        eprintln!("[TEST] PASS: extend_selection_to_cursor works");
    }

    #[test]
    fn test_extend_selection_backward() {
        eprintln!("[TEST] test_extend_selection_backward");
        let buffer = EditBuffer::with_text("Hello, World!");
        let mut view = EditorView::new(buffer);

        // Start selection in middle
        view.edit_buffer_mut().set_cursor_by_offset(7);
        view.start_selection();
        eprintln!("[TEST] Started selection at offset 7");

        // Extend backward
        view.edit_buffer_mut().set_cursor_by_offset(0);
        view.extend_selection_to_cursor();

        let sel = view.selection.unwrap();
        eprintln!(
            "[TEST] Backward selection: start={} end={}",
            sel.start, sel.end
        );

        // start > end is valid (indicates backward selection)
        assert_eq!(sel.start, 7);
        assert_eq!(sel.end, 0);

        eprintln!("[TEST] PASS: Selection can extend backward");
    }

    #[test]
    fn test_selected_text() {
        eprintln!("[TEST] test_selected_text");
        let buffer = EditBuffer::with_text("Hello, World!");
        let mut view = EditorView::new(buffer);

        // No selection initially
        assert!(view.selected_text().is_none());
        eprintln!("[TEST] No selection initially");

        // Create selection
        view.set_selection(0, 5); // "Hello"

        let text = view.selected_text();
        eprintln!("[TEST] Selected text: {text:?}");

        assert_eq!(text, Some("Hello".to_string()));

        // Backward selection should also work
        view.set_selection(13, 7); // "World!" backward
        let text = view.selected_text();
        eprintln!("[TEST] Backward selection text: {text:?}");
        assert_eq!(text, Some("World!".to_string()));

        // Empty selection (start == end) should return None
        view.set_selection(5, 5);
        let text = view.selected_text();
        eprintln!("[TEST] Empty selection text: {text:?}");
        assert!(text.is_none(), "Empty selection should return None");

        eprintln!("[TEST] PASS: selected_text returns correct content");
    }

    #[test]
    fn test_selection_with_cursor_movement() {
        eprintln!("[TEST] test_selection_with_cursor_movement");
        let buffer = EditBuffer::with_text("Line 1\nLine 2\nLine 3");
        let mut view = EditorView::new(buffer);

        // Start selection
        view.start_selection();
        eprintln!("[TEST] Started selection at line 0");

        // Move down and extend
        view.edit_buffer_mut().move_down();
        view.extend_selection_to_cursor();

        let sel = view.selection.unwrap();
        eprintln!(
            "[TEST] Selection after move_down: {} to {}",
            sel.start, sel.end
        );

        // Should have selected from start to current position
        let text = view.selected_text().unwrap();
        eprintln!("[TEST] Selected: {text:?}");

        assert!(text.contains("Line"));

        eprintln!("[TEST] PASS: Selection works with cursor movement");
    }

    #[test]
    fn test_selection_follow_cursor_mode() {
        eprintln!("[TEST] test_selection_follow_cursor_mode");
        let buffer = EditBuffer::with_text("Hello, World!");
        let mut view = EditorView::new(buffer);

        // Enable selection follow cursor
        view.set_selection_follow_cursor(true);
        view.set_selection(0, 0);

        eprintln!("[TEST] Selection follow cursor enabled");

        // Move cursor - selection should extend automatically via scroll_to_cursor
        view.edit_buffer_mut().set_cursor_by_offset(5);
        view.scroll_to_cursor(80, 24); // This triggers selection follow

        let sel = view.selection.unwrap();
        eprintln!("[TEST] After cursor move: selection end={}", sel.end);

        assert_eq!(sel.end, 5, "Selection should follow cursor");

        eprintln!("[TEST] PASS: selection_follow_cursor mode works");
    }
}
