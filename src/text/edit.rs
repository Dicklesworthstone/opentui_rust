//! Editable text buffer with cursor and undo/redo.
//!
//! This module provides [`EditBuffer`], which wraps a [`TextBuffer`] with
//! editing capabilities including cursor movement, text insertion/deletion,
//! and undo/redo history.
//!
//! # Examples
//!
//! ```
//! use opentui::EditBuffer;
//!
//! let mut buf = EditBuffer::with_text("Hello World");
//!
//! // Move cursor to end of line and delete backward
//! buf.move_to_line_end();
//! buf.delete_backward(); // Removes 'd'
//! buf.commit(); // Create undo checkpoint
//! assert_eq!(buf.text(), "Hello Worl");
//!
//! // Undo restores deleted text
//! buf.undo();
//! assert_eq!(buf.text(), "Hello World");
//! ```

// Iterator patterns are clearer in their current form
#![allow(clippy::while_let_on_iterator)]
// if-let-else is clearer than map_or for complex logic
#![allow(clippy::option_if_let_else)]

use crate::highlight::HighlightedBuffer;
use crate::text::TextBuffer;

/// Cursor position in the buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    /// Character offset in the buffer.
    pub offset: usize,
    /// Line number (0-indexed).
    pub row: usize,
    /// Column number (0-indexed).
    pub col: usize,
}

impl Cursor {
    /// Create a new cursor at position.
    #[must_use]
    pub fn new(offset: usize, row: usize, col: usize) -> Self {
        Self { offset, row, col }
    }

    /// Create a cursor at the beginning.
    #[must_use]
    pub fn start() -> Self {
        Self::default()
    }
}

/// Cursor position info with offset and visual column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: usize,
    pub col: usize,
    pub offset: usize,
}

/// An edit operation for undo/redo.
#[derive(Clone, Debug)]
enum EditOp {
    Insert { offset: usize, text: String },
    Delete { offset: usize, text: String },
}

impl EditOp {
    fn invert(&self) -> Self {
        match self {
            Self::Insert { offset, text } => Self::Delete {
                offset: *offset,
                text: text.clone(),
            },
            Self::Delete { offset, text } => Self::Insert {
                offset: *offset,
                text: text.clone(),
            },
        }
    }
}

/// Default maximum number of undo groups to retain.
const DEFAULT_MAX_HISTORY_DEPTH: usize = 1000;

/// Edit history for undo/redo with bounded memory usage.
#[derive(Clone, Debug)]
struct History {
    undo_stack: Vec<Vec<EditOp>>,
    redo_stack: Vec<Vec<EditOp>>,
    current_group: Vec<EditOp>,
    /// Maximum number of undo groups to retain. Oldest entries are dropped when exceeded.
    max_depth: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_group: Vec::new(),
            max_depth: DEFAULT_MAX_HISTORY_DEPTH,
        }
    }
}

impl History {
    fn new() -> Self {
        Self::default()
    }

    /// Create a history with a custom maximum depth.
    fn with_max_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
            ..Self::default()
        }
    }

    fn push(&mut self, op: EditOp) {
        self.current_group.push(op);
        self.redo_stack.clear();
    }

    fn commit(&mut self) {
        if !self.current_group.is_empty() {
            self.undo_stack
                .push(std::mem::take(&mut self.current_group));
            // Enforce depth limit by dropping oldest entries
            if self.undo_stack.len() > self.max_depth {
                let excess = self.undo_stack.len() - self.max_depth;
                self.undo_stack.drain(..excess);
            }
        }
    }

    fn pop_undo(&mut self) -> Option<Vec<EditOp>> {
        self.commit();
        self.undo_stack.pop()
    }

    fn push_redo(&mut self, ops: Vec<EditOp>) {
        self.redo_stack.push(ops);
    }

    fn pop_redo(&mut self) -> Option<Vec<EditOp>> {
        self.redo_stack.pop()
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty() || !self.current_group.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_group.clear();
    }
}

/// Text buffer with editing operations, cursor, and undo/redo.
///
/// `EditBuffer` is the primary type for text editing. It tracks cursor
/// position, maintains undo/redo history, and provides operations for:
///
/// - **Cursor movement**: Lines, words, characters, document bounds
/// - **Text editing**: Insert, delete, backspace with cursor tracking
/// - **Line operations**: Duplicate, move, delete lines
/// - **History**: Grouped undo/redo with configurable depth limit
///
/// # History Management
///
/// Edit operations are grouped automatically. Call [`commit`](Self::commit)
/// to force a group boundary (e.g., after a pause in typing). The history depth
/// is bounded (default 1000 groups) to limit memory usage.
#[derive(Default)]
pub struct EditBuffer {
    buffer: HighlightedBuffer,
    cursor: Cursor,
    history: History,
}

impl EditBuffer {
    /// Create a new empty edit buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an edit buffer with initial text.
    #[must_use]
    pub fn with_text(text: &str) -> Self {
        Self {
            buffer: HighlightedBuffer::new(TextBuffer::with_text(text)),
            cursor: Cursor::start(),
            history: History::new(),
        }
    }

    /// Create an edit buffer with a custom maximum undo history depth.
    ///
    /// The default is 1000 undo groups. Set a lower value for memory-constrained
    /// environments or a higher value for documents that need extensive undo history.
    #[must_use]
    pub fn with_max_history_depth(max_depth: usize) -> Self {
        Self {
            buffer: HighlightedBuffer::new(TextBuffer::new()),
            cursor: Cursor::start(),
            history: History::with_max_depth(max_depth),
        }
    }

    /// Set the maximum undo history depth.
    ///
    /// If the current history exceeds the new depth, oldest entries will be
    /// pruned on the next commit.
    pub fn set_max_history_depth(&mut self, max_depth: usize) {
        self.history.max_depth = max_depth;
    }

    /// Get the current maximum undo history depth.
    #[must_use]
    pub fn max_history_depth(&self) -> usize {
        self.history.max_depth
    }

    /// Get the underlying text buffer.
    #[must_use]
    pub fn buffer(&self) -> &TextBuffer {
        self.buffer.buffer()
    }

    /// Get the full text content.
    #[must_use]
    pub fn text(&self) -> String {
        self.buffer.buffer().to_string()
    }

    /// Replace the entire text, resetting cursor and history.
    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(text);
        self.cursor = Cursor::start();
        self.history.clear();
        self.update_cursor_position();
    }

    /// Get mutable access to the text buffer.
    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        self.buffer.buffer_mut()
    }

    /// Get the highlighted buffer.
    #[must_use]
    pub fn highlighted_buffer(&self) -> &HighlightedBuffer {
        &self.buffer
    }

    /// Get mutable access to the highlighted buffer.
    pub fn highlighted_buffer_mut(&mut self) -> &mut HighlightedBuffer {
        &mut self.buffer
    }

    /// Get the current cursor position.
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Set the cursor position.
    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
        self.clamp_cursor();
    }

    /// Set the cursor by character offset.
    pub fn set_cursor_by_offset(&mut self, offset: usize) {
        self.cursor.offset = offset.min(self.buffer.len_chars());
        self.update_cursor_position();
    }

    /// Get cursor position info.
    #[must_use]
    pub fn get_cursor_position(&self) -> CursorPosition {
        CursorPosition {
            row: self.cursor.row,
            col: self.cursor.col,
            offset: self.cursor.offset,
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor.offset > 0 {
            self.cursor.offset -= 1;
            self.update_cursor_position();
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if self.cursor.offset < self.buffer.len_chars() {
            self.cursor.offset += 1;
            self.update_cursor_position();
        }
    }

    /// Move cursor up.
    pub fn move_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.update_cursor_from_row_col();
        }
    }

    /// Move cursor down.
    pub fn move_down(&mut self) {
        if self.cursor.row + 1 < self.buffer.len_lines() {
            self.cursor.row += 1;
            self.update_cursor_from_row_col();
        }
    }

    /// Move cursor to start of line.
    pub fn move_to_line_start(&mut self) {
        self.cursor.col = 0;
        self.update_cursor_from_row_col();
    }

    /// Move cursor to end of line.
    pub fn move_to_line_end(&mut self) {
        if let Some(line) = self.buffer.line(self.cursor.row) {
            self.cursor.col = line.trim_end_matches('\n').chars().count();
            self.update_cursor_from_row_col();
        }
    }

    /// Move to specific row and column.
    pub fn move_to(&mut self, row: usize, col: usize) {
        self.cursor.row = row.min(self.buffer.len_lines().saturating_sub(1));
        self.cursor.col = col;
        self.update_cursor_from_row_col();
    }

    /// Jump to a specific line (start of line).
    pub fn goto_line(&mut self, row: usize) {
        let row = row.min(self.buffer.len_lines().saturating_sub(1));
        self.cursor.row = row;
        self.cursor.col = 0;
        self.update_cursor_from_row_col();
    }

    /// Insert text at cursor.
    pub fn insert(&mut self, text: &str) {
        let offset = self.cursor.offset;
        self.buffer.rope_mut().insert(offset, text);
        let line_delta = text.chars().filter(|&ch| ch == '\n').count();
        let start_row = self.cursor.row;
        let end_row = start_row.saturating_add(line_delta + 1);
        self.buffer.mark_dirty(start_row, end_row);
        self.history.push(EditOp::Insert {
            offset,
            text: text.to_string(),
        });

        self.cursor.offset += text.chars().count();
        self.update_cursor_position();
    }

    /// Delete character before cursor.
    pub fn delete_backward(&mut self) {
        if self.cursor.offset == 0 {
            return;
        }

        let start = self.cursor.offset - 1;
        let deleted = self
            .buffer
            .rope()
            .slice(start..self.cursor.offset)
            .to_string();

        self.buffer.rope_mut().remove(start..self.cursor.offset);
        self.buffer
            .mark_dirty(self.cursor.row.saturating_sub(1), self.cursor.row + 1); // might affect prev line
        self.history.push(EditOp::Delete {
            offset: start,
            text: deleted,
        });

        self.cursor.offset = start;
        self.update_cursor_position();
    }

    /// Delete character after cursor.
    pub fn delete_forward(&mut self) {
        if self.cursor.offset >= self.buffer.len_chars() {
            return;
        }

        let end = self.cursor.offset + 1;
        let deleted = self
            .buffer
            .rope()
            .slice(self.cursor.offset..end)
            .to_string();

        self.buffer.rope_mut().remove(self.cursor.offset..end);
        let start_row = self.cursor.row;
        let end_row = if deleted.contains('\n') {
            start_row.saturating_add(2)
        } else {
            start_row.saturating_add(1)
        };
        self.buffer.mark_dirty(start_row, end_row);
        self.history.push(EditOp::Delete {
            offset: self.cursor.offset,
            text: deleted,
        });

        self.update_cursor_position();
    }

    /// Delete a range between two cursors.
    pub fn delete_range(&mut self, start: Cursor, end: Cursor) {
        let start_offset = start.offset.min(end.offset);
        let end_offset = start.offset.max(end.offset);
        self.delete_range_offsets(start_offset, end_offset);
    }

    /// Delete a range between character offsets.
    pub fn delete_range_offsets(&mut self, start: usize, end: usize) {
        if start >= end || start >= self.buffer.len_chars() {
            return;
        }
        let end = end.min(self.buffer.len_chars());
        let (start_row, end_row, deleted) = {
            let rope = self.buffer.rope();
            let start_row = rope.char_to_line(start);
            let end_row = rope.char_to_line(end.saturating_sub(1));
            let deleted = rope.slice(start..end).to_string();
            (start_row, end_row, deleted)
        };
        self.buffer.rope_mut().remove(start..end);

        self.buffer.mark_dirty(start_row, end_row.saturating_add(1));

        self.history.push(EditOp::Delete {
            offset: start,
            text: deleted,
        });
        self.cursor.offset = start;
        self.update_cursor_position();
    }

    /// Delete the current line (including trailing newline if present).
    pub fn delete_line(&mut self) {
        let line_start = self.buffer.rope().line_to_char(self.cursor.row);
        if let Some(line) = self.buffer.rope().line(self.cursor.row) {
            let line_chars = line.len_chars();
            let line_end = line_start + line_chars;
            self.delete_range_offsets(line_start, line_end);
        }
    }

    /// Duplicate the current line (insert copy below).
    pub fn duplicate_line(&mut self) {
        let line_start = self.buffer.rope().line_to_char(self.cursor.row);
        if let Some(line) = self.buffer.rope().line(self.cursor.row) {
            let line_text = line.to_string();
            // Insert at the end of the current line
            let insert_pos = line_start + line.len_chars();

            // Build the text to insert:
            // - If original line has newline: insert the line as-is (already ends with \n)
            // - If original line has no newline (last line): prepend \n, don't append
            let text_to_insert = if line_text.ends_with('\n') {
                line_text.clone()
            } else {
                format!("\n{line_text}")
            };

            self.buffer.rope_mut().insert(insert_pos, &text_to_insert);
            self.buffer.mark_dirty(self.cursor.row, self.cursor.row + 2);

            self.history.push(EditOp::Insert {
                offset: insert_pos,
                text: text_to_insert,
            });
            // Move cursor to the duplicated line
            self.cursor.row += 1;
            self.update_cursor_from_row_col();
        }
    }

    /// Move the current line up (swap with the line above).
    pub fn move_line_up(&mut self) {
        if self.cursor.row == 0 {
            return;
        }

        let target_row = self.cursor.row - 1;
        let target_col = self.cursor.col;
        let current_line_start = self.buffer.rope().line_to_char(self.cursor.row);
        let prev_line_start = self.buffer.rope().line_to_char(target_row);

        if let (Some(current_line), Some(prev_line)) = (
            self.buffer.rope().line(self.cursor.row),
            self.buffer.rope().line(target_row),
        ) {
            let current_text = current_line.to_string();
            let prev_text = prev_line.to_string();

            // Delete from start of previous line to end of current line
            let end_pos = current_line_start + current_line.len_chars();
            self.delete_range_offsets(prev_line_start, end_pos);

            // Insert current line first, then previous line
            let new_text = if current_text.ends_with('\n') {
                format!("{current_text}{prev_text}")
            } else if prev_text.ends_with('\n') {
                format!("{current_text}\n{}", prev_text.trim_end_matches('\n'))
            } else {
                format!("{current_text}\n{prev_text}")
            };

            self.buffer.rope_mut().insert(prev_line_start, &new_text);
            self.buffer.mark_dirty(target_row, target_row + 2);

            self.history.push(EditOp::Insert {
                offset: prev_line_start,
                text: new_text,
            });

            // Update cursor to the new position (one row up, same column)
            self.cursor.row = target_row;
            self.cursor.col = target_col;
            self.update_cursor_from_row_col();
        }
    }

    /// Move the current line down (swap with the line below).
    pub fn move_line_down(&mut self) {
        let total_lines = self.buffer.len_lines();
        if self.cursor.row >= total_lines.saturating_sub(1) {
            return;
        }

        let target_row = self.cursor.row + 1;
        let target_col = self.cursor.col;
        let current_line_start = self.buffer.rope().line_to_char(self.cursor.row);
        let next_line_start = self.buffer.rope().line_to_char(target_row);

        if let (Some(current_line), Some(next_line)) = (
            self.buffer.rope().line(self.cursor.row),
            self.buffer.rope().line(target_row),
        ) {
            let current_text = current_line.to_string();
            let next_text = next_line.to_string();

            // Delete from start of current line to end of next line
            let end_pos = next_line_start + next_line.len_chars();

            self.delete_range_offsets(current_line_start, end_pos);

            // Insert next line first, then current line
            let new_text = if next_text.ends_with('\n') {
                format!("{next_text}{current_text}")
            } else if current_text.ends_with('\n') {
                format!("{next_text}\n{}", current_text.trim_end_matches('\n'))
            } else {
                format!("{next_text}\n{current_text}")
            };

            self.buffer.rope_mut().insert(current_line_start, &new_text);
            self.buffer.mark_dirty(self.cursor.row, self.cursor.row + 2);

            self.history.push(EditOp::Insert {
                offset: current_line_start,
                text: new_text,
            });

            // Update cursor to the new position (one row down, same column)
            self.cursor.row = target_row;
            self.cursor.col = target_col;
            self.update_cursor_from_row_col();
        }
    }

    /// Replace the entire text, clearing history.
    pub fn replace_text(&mut self, text: &str) {
        self.set_text(text);
    }

    /// Get the next word boundary (character offset).
    #[must_use]
    pub fn get_next_word_boundary(&self) -> usize {
        let text = self.buffer.to_string();
        let mut chars = text.chars().enumerate().skip(self.cursor.offset);
        let mut in_word = false;
        let mut last_idx = self.cursor.offset;

        while let Some((idx, ch)) = chars.next() {
            let word_char = ch.is_alphanumeric() || ch == '_';
            if in_word && !word_char {
                return idx;
            }
            if !in_word && word_char {
                in_word = true;
            }
            last_idx = idx + 1;
        }
        last_idx
    }

    /// Get the previous word boundary (character offset).
    #[must_use]
    pub fn get_prev_word_boundary(&self) -> usize {
        let text = self.buffer.to_string();
        let chars: Vec<char> = text.chars().collect();
        if self.cursor.offset == 0 {
            return 0;
        }
        let mut idx = self.cursor.offset.min(chars.len());

        // Skip any non-word characters first
        while idx > 0 {
            let ch = chars[idx - 1];
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            idx -= 1;
        }
        // Then skip word characters
        while idx > 0 {
            let ch = chars[idx - 1];
            if !(ch.is_alphanumeric() || ch == '_') {
                break;
            }
            idx -= 1;
        }
        idx
    }

    /// Move cursor to the next word boundary.
    pub fn move_word_right(&mut self) {
        let boundary = self.get_next_word_boundary();
        self.set_cursor_by_offset(boundary);
    }

    /// Move cursor to the previous word boundary.
    pub fn move_word_left(&mut self) {
        let boundary = self.get_prev_word_boundary();
        self.set_cursor_by_offset(boundary);
    }

    /// Delete from cursor to the next word boundary.
    pub fn delete_word_forward(&mut self) {
        let end = self.get_next_word_boundary();
        if end > self.cursor.offset {
            self.delete_range_offsets(self.cursor.offset, end);
        }
    }

    /// Delete from cursor to the previous word boundary.
    pub fn delete_word_backward(&mut self) {
        let start = self.get_prev_word_boundary();
        if start < self.cursor.offset {
            self.delete_range_offsets(start, self.cursor.offset);
        }
    }

    /// Get end of line offset for current line.
    #[must_use]
    pub fn get_eol(&self) -> usize {
        if let Some(line) = self.buffer.rope().line(self.cursor.row) {
            let line_chars = line.len_chars();
            let has_newline = line_chars > 0 && line.char(line_chars - 1) == '\n';
            let line_len = if has_newline {
                line_chars - 1
            } else {
                line_chars
            };
            let line_start = self.buffer.rope().line_to_char(self.cursor.row);
            line_start + line_len
        } else {
            self.cursor.offset
        }
    }

    /// Undo the last edit.
    pub fn undo(&mut self) -> bool {
        let Some(ops) = self.history.pop_undo() else {
            return false;
        };

        let mut redo_ops = Vec::new();
        for op in ops.into_iter().rev() {
            self.apply_op(&op.invert());
            redo_ops.push(op);
        }
        redo_ops.reverse();
        self.history.push_redo(redo_ops);

        true
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> bool {
        let Some(ops) = self.history.pop_redo() else {
            return false;
        };

        for op in &ops {
            self.apply_op(op);
        }
        self.history.undo_stack.push(ops);

        true
    }

    /// Check if undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Check if redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Commit current edits as an undo group.
    pub fn commit(&mut self) {
        self.history.commit();
    }

    /// Clear the undo/redo history.
    ///
    /// This removes all undo and redo entries. Useful when loading new content
    /// where previous history is no longer relevant.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    fn apply_op(&mut self, op: &EditOp) {
        match op {
            EditOp::Insert { offset, text } => {
                self.buffer.rope_mut().insert(*offset, text);

                let row = self.buffer.rope().char_to_line(*offset);
                let line_delta = text.chars().filter(|&ch| ch == '\n').count();
                self.buffer
                    .mark_dirty(row, row.saturating_add(line_delta + 1));

                self.cursor.offset = offset + text.chars().count();
            }
            EditOp::Delete { offset, text } => {
                let end = offset + text.chars().count();
                let (start_row, end_row) = {
                    let rope = self.buffer.rope();
                    let start_row = rope.char_to_line(*offset);
                    let end_row = rope.char_to_line(end.saturating_sub(1));
                    (start_row, end_row)
                };
                self.buffer.rope_mut().remove(*offset..end);
                self.buffer.mark_dirty(start_row, end_row.saturating_add(1));

                self.cursor.offset = *offset;
            }
        }
        self.update_cursor_position();
    }

    fn update_cursor_position(&mut self) {
        let rope = self.buffer.rope();
        self.cursor.row = rope
            .inner()
            .char_to_line(self.cursor.offset.min(rope.len_chars()));
        let line_start = rope.line_to_char(self.cursor.row);
        self.cursor.col = self.cursor.offset.saturating_sub(line_start);
    }

    fn update_cursor_from_row_col(&mut self) {
        let rope = self.buffer.rope();
        let line_start = rope.line_to_char(self.cursor.row);

        if let Some(line) = rope.line(self.cursor.row) {
            let line_chars = line.len_chars();
            // Only exclude newline if actually present at end of line
            let has_newline = line_chars > 0 && line.char(line_chars - 1) == '\n';
            let line_len = if has_newline {
                line_chars - 1
            } else {
                line_chars
            };
            self.cursor.col = self.cursor.col.min(line_len);
        }

        self.cursor.offset = line_start + self.cursor.col;
    }

    fn clamp_cursor(&mut self) {
        self.cursor.offset = self.cursor.offset.min(self.buffer.len_chars());
        self.update_cursor_position();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_basic() {
        let mut edit = EditBuffer::new();
        edit.insert("Hello");
        assert_eq!(edit.text(), "Hello");
        assert_eq!(edit.cursor().offset, 5);
    }

    #[test]
    fn test_edit_delete() {
        let mut edit = EditBuffer::with_text("Hello");
        edit.move_to(0, 5);
        edit.delete_backward();
        assert_eq!(edit.text(), "Hell");
    }

    #[test]
    fn test_edit_undo() {
        let mut edit = EditBuffer::new();
        edit.insert("Hello");
        edit.commit();
        edit.insert(" World");
        edit.commit();
        assert_eq!(edit.text(), "Hello World");

        edit.undo();
        assert_eq!(edit.text(), "Hello");

        edit.undo();
        assert_eq!(edit.text(), "");
    }

    #[test]
    fn test_edit_redo() {
        let mut edit = EditBuffer::new();
        edit.insert("Hello");
        edit.commit();

        edit.undo();
        assert_eq!(edit.text(), "");

        edit.redo();
        assert_eq!(edit.text(), "Hello");
    }

    #[test]
    fn test_cursor_movement() {
        let mut edit = EditBuffer::with_text("Line 1\nLine 2");
        edit.move_to(0, 0);
        assert_eq!(edit.cursor().row, 0);

        edit.move_down();
        assert_eq!(edit.cursor().row, 1);

        edit.move_up();
        assert_eq!(edit.cursor().row, 0);
    }

    #[test]
    fn test_history_depth_limit() {
        let mut edit = EditBuffer::with_max_history_depth(3);
        assert_eq!(edit.max_history_depth(), 3);

        // Add 5 undo groups
        for i in 0..5 {
            edit.insert(&format!("{i}"));
            edit.commit();
        }
        assert_eq!(edit.text(), "01234");

        // Should only be able to undo 3 times (depth limit)
        assert!(edit.undo()); // undo "4"
        assert!(edit.undo()); // undo "3"
        assert!(edit.undo()); // undo "2"
        assert!(!edit.undo()); // no more undo available

        // Text should be "01" (groups 0 and 1 were pruned)
        assert_eq!(edit.text(), "01");
    }

    #[test]
    fn test_set_max_history_depth() {
        let mut edit = EditBuffer::new();
        assert_eq!(edit.max_history_depth(), 1000); // default

        edit.set_max_history_depth(50);
        assert_eq!(edit.max_history_depth(), 50);
    }

    #[test]
    fn test_delete_range_offsets() {
        let mut edit = EditBuffer::with_text("Hello, world!");
        // Deleting positions 5-6 removes "," and " "
        edit.delete_range_offsets(5, 6);
        assert_eq!(edit.text(), "Hello world!");
    }

    #[test]
    fn test_delete_line() {
        let mut edit = EditBuffer::with_text("Line 1\nLine 2\nLine 3");
        edit.move_to(1, 0);
        edit.delete_line();
        assert_eq!(edit.text(), "Line 1\nLine 3");
    }

    #[test]
    fn test_word_boundaries() {
        let mut edit = EditBuffer::with_text("hello world");
        edit.set_cursor_by_offset(0);
        assert_eq!(edit.get_next_word_boundary(), 5);
        edit.set_cursor_by_offset(6);
        assert_eq!(edit.get_prev_word_boundary(), 0);
    }

    #[test]
    fn test_move_word_right() {
        let mut edit = EditBuffer::with_text("hello world test");
        edit.set_cursor_by_offset(0);
        edit.move_word_right();
        assert_eq!(edit.cursor().offset, 5);
        edit.move_word_right();
        assert_eq!(edit.cursor().offset, 11);
    }

    #[test]
    fn test_move_word_left() {
        let mut edit = EditBuffer::with_text("hello world test");
        edit.set_cursor_by_offset(16);
        edit.move_word_left();
        assert_eq!(edit.cursor().offset, 12);
        edit.move_word_left();
        assert_eq!(edit.cursor().offset, 6);
    }

    #[test]
    fn test_delete_word_forward() {
        let mut edit = EditBuffer::with_text("hello world");
        edit.set_cursor_by_offset(0);
        edit.delete_word_forward();
        assert_eq!(edit.text(), " world");
    }

    #[test]
    fn test_delete_word_backward() {
        let mut edit = EditBuffer::with_text("hello world");
        edit.set_cursor_by_offset(11);
        edit.delete_word_backward();
        assert_eq!(edit.text(), "hello ");
    }

    #[test]
    fn test_goto_line() {
        let mut edit = EditBuffer::with_text("Line 1\nLine 2\nLine 3");
        edit.goto_line(2);
        assert_eq!(edit.cursor().row, 2);
        assert_eq!(edit.cursor().col, 0);
    }

    #[test]
    fn test_duplicate_line() {
        eprintln!("[TEST] test_duplicate_line: Testing line duplication");
        let mut edit = EditBuffer::with_text("Line 1\nLine 2\nLine 3");
        edit.goto_line(1);
        eprintln!("[TEST] Before duplicate: {:?}", edit.text());
        eprintln!("[TEST] Cursor at row: {}", edit.cursor().row);

        edit.duplicate_line();

        eprintln!("[TEST] After duplicate: {:?}", edit.text());
        eprintln!("[TEST] Cursor at row: {}", edit.cursor().row);
        assert_eq!(edit.text(), "Line 1\nLine 2\nLine 2\nLine 3");
        assert_eq!(
            edit.cursor().row,
            2,
            "Cursor should move to duplicated line"
        );
        eprintln!("[TEST] SUCCESS: Line duplication works correctly");
    }

    #[test]
    fn test_duplicate_last_line() {
        eprintln!(
            "[TEST] test_duplicate_last_line: Testing last line duplication (no trailing newline)"
        );
        let mut edit = EditBuffer::with_text("Line 1\nLine 2");
        edit.goto_line(1);
        eprintln!("[TEST] Before duplicate: {:?}", edit.text());

        edit.duplicate_line();

        eprintln!("[TEST] After duplicate: {:?}", edit.text());
        assert_eq!(edit.text(), "Line 1\nLine 2\nLine 2");
        eprintln!("[TEST] SUCCESS: Last line duplication works correctly");
    }

    #[test]
    fn test_move_line_up() {
        eprintln!("[TEST] test_move_line_up: Testing moving line up");
        let mut edit = EditBuffer::with_text("Line 1\nLine 2\nLine 3");
        edit.goto_line(1);
        eprintln!("[TEST] Before move: {:?}", edit.text());
        eprintln!("[TEST] Cursor at row: {}", edit.cursor().row);

        edit.move_line_up();

        eprintln!("[TEST] After move: {:?}", edit.text());
        eprintln!("[TEST] Cursor at row: {}", edit.cursor().row);
        assert_eq!(edit.text(), "Line 2\nLine 1\nLine 3");
        assert_eq!(edit.cursor().row, 0, "Cursor should follow the moved line");
        eprintln!("[TEST] SUCCESS: Move line up works correctly");
    }

    #[test]
    fn test_move_line_up_at_top() {
        eprintln!("[TEST] test_move_line_up_at_top: Testing move line up at first line (no-op)");
        let mut edit = EditBuffer::with_text("Line 1\nLine 2");
        edit.goto_line(0);

        edit.move_line_up();

        assert_eq!(edit.text(), "Line 1\nLine 2", "Should be unchanged");
        assert_eq!(edit.cursor().row, 0);
        eprintln!("[TEST] SUCCESS: Move line up at top is a no-op");
    }

    #[test]
    fn test_move_line_down() {
        eprintln!("[TEST] test_move_line_down: Testing moving line down");
        let mut edit = EditBuffer::with_text("Line 1\nLine 2\nLine 3");
        edit.goto_line(1);
        eprintln!("[TEST] Before move: {:?}", edit.text());
        eprintln!("[TEST] Cursor at row: {}", edit.cursor().row);

        edit.move_line_down();

        eprintln!("[TEST] After move: {:?}", edit.text());
        eprintln!("[TEST] Cursor at row: {}", edit.cursor().row);
        assert_eq!(edit.text(), "Line 1\nLine 3\nLine 2");
        assert_eq!(edit.cursor().row, 2, "Cursor should follow the moved line");
        eprintln!("[TEST] SUCCESS: Move line down works correctly");
    }

    #[test]
    fn test_move_line_down_at_bottom() {
        eprintln!(
            "[TEST] test_move_line_down_at_bottom: Testing move line down at last line (no-op)"
        );
        let mut edit = EditBuffer::with_text("Line 1\nLine 2");
        edit.goto_line(1);

        edit.move_line_down();

        assert_eq!(edit.text(), "Line 1\nLine 2", "Should be unchanged");
        assert_eq!(edit.cursor().row, 1);
        eprintln!("[TEST] SUCCESS: Move line down at bottom is a no-op");
    }

    #[test]
    fn test_line_operations_with_undo() {
        eprintln!("[TEST] test_line_operations_with_undo: Testing undo for line operations");
        let mut edit = EditBuffer::with_text("Line 1\nLine 2\nLine 3");
        let original = edit.text().clone();
        edit.goto_line(1);

        // Duplicate and undo
        edit.duplicate_line();
        assert_ne!(edit.text(), original);
        edit.undo();
        assert_eq!(
            edit.text(),
            original,
            "Undo should restore original after duplicate"
        );

        // Move up and undo
        edit.goto_line(1);
        edit.move_line_up();
        assert_ne!(edit.text(), original);
        edit.undo();
        edit.undo(); // Need two undos - one for delete, one for insert
        assert_eq!(
            edit.text(),
            original,
            "Undo should restore original after move up"
        );

        eprintln!("[TEST] SUCCESS: Undo works for line operations");
    }
}
