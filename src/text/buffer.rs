//! Styled text buffer with highlighting support.
//!
//! This module provides [`TextBuffer`], a rope-backed text storage that
//! supports styled segments and syntax highlighting. Use this for read-only
//! or display-oriented text. For editing with cursor and undo, see
//! [`EditBuffer`](super::EditBuffer).

use crate::highlight::SyntaxStyleRegistry;
use crate::style::Style;
use crate::text::rope::RopeWrapper;
use crate::text::segment::{StyledChunk, StyledSegment};
use crate::unicode::WidthMethod;
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct MemEntry {
    data: String,
    owned: bool,
}

#[derive(Clone, Debug, Default)]
struct MemRegistry {
    entries: Vec<Option<MemEntry>>,
    free_list: Vec<u32>,
}

impl MemRegistry {
    fn register(&mut self, data: &str, owned: bool) -> u32 {
        if let Some(id) = self.free_list.pop() {
            let idx = (id - 1) as usize;
            self.entries[idx] = Some(MemEntry {
                data: data.to_string(),
                owned,
            });
            return id;
        }

        self.entries.push(Some(MemEntry {
            data: data.to_string(),
            owned,
        }));
        self.entries.len() as u32
    }

    fn replace(&mut self, id: u32, data: &str, owned: bool) {
        if id == 0 {
            return;
        }
        let idx = id.saturating_sub(1) as usize;
        if let Some(slot) = self.entries.get_mut(idx) {
            *slot = Some(MemEntry {
                data: data.to_string(),
                owned,
            });
        }
    }

    fn get(&self, id: u32) -> Option<&str> {
        if id == 0 {
            return None;
        }
        let idx = id.saturating_sub(1) as usize;
        self.entries
            .get(idx)
            .and_then(|entry| entry.as_ref().map(|m| m.data.as_str()))
    }
}

/// Text buffer with styled segments and highlights.
///
/// `TextBuffer` uses a rope data structure internally for O(log n) insertions
/// and deletions, making it suitable for large documents. It also supports:
///
/// - Styled segments for syntax highlighting or markup
/// - Memory registry for efficient string deduplication
/// - Tab width configuration
/// - Unicode width calculation methods
///
/// For editing with cursor movement and undo/redo, wrap this in an
/// [`EditBuffer`](super::EditBuffer).
#[derive(Clone, Debug, Default)]
pub struct TextBuffer {
    rope: RopeWrapper,
    segments: Vec<StyledSegment>,
    default_style: Style,
    tab_width: u8,
    mem_registry: MemRegistry,
    width_method: WidthMethod,
    syntax_styles: Option<Arc<SyntaxStyleRegistry>>,
    revision: u64,
}

impl TextBuffer {
    /// Create an empty text buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rope: RopeWrapper::new(),
            segments: Vec::new(),
            default_style: Style::NONE,
            tab_width: 4,
            mem_registry: MemRegistry::default(),
            width_method: WidthMethod::default(),
            syntax_styles: None,
            revision: 0,
        }
    }

    /// Create a buffer with initial text.
    #[must_use]
    pub fn with_text(text: &str) -> Self {
        Self {
            rope: RopeWrapper::from_str(text),
            segments: Vec::new(),
            default_style: Style::NONE,
            tab_width: 4,
            mem_registry: MemRegistry::default(),
            width_method: WidthMethod::default(),
            syntax_styles: None,
            revision: 0,
        }
    }

    /// Set the default style for unstyled text.
    pub fn set_default_style(&mut self, style: Style) {
        self.default_style = style;
    }

    /// Get the default style.
    #[must_use]
    pub fn default_style(&self) -> Style {
        self.default_style
    }

    /// Set tab width.
    pub fn set_tab_width(&mut self, width: u8) {
        self.tab_width = width;
    }

    /// Get tab width.
    #[must_use]
    pub fn tab_width(&self) -> u8 {
        self.tab_width
    }

    /// Set width calculation method for this buffer.
    pub fn set_width_method(&mut self, method: WidthMethod) {
        self.width_method = method;
    }

    /// Get width calculation method.
    #[must_use]
    pub fn width_method(&self) -> WidthMethod {
        self.width_method
    }

    /// Attach a syntax style registry for style-id based highlights.
    pub fn set_syntax_styles(&mut self, registry: Arc<SyntaxStyleRegistry>) {
        self.syntax_styles = Some(registry);
    }

    /// Clear the syntax style registry.
    pub fn clear_syntax_styles(&mut self) {
        self.syntax_styles = None;
    }

    /// Set the text content, clearing all segments.
    pub fn set_text(&mut self, text: &str) {
        self.rope.replace(text);
        self.segments.clear();
        self.bump_revision();
    }

    /// Append text to the buffer.
    pub fn append(&mut self, text: &str) {
        self.rope.append(text);
        self.bump_revision();
    }

    /// Set styled text content from chunks.
    pub fn set_styled_text(&mut self, chunks: &[StyledChunk<'_>]) {
        self.rope.clear();
        self.segments.clear();
        self.bump_revision();

        let mut offset = 0;
        for chunk in chunks {
            let start = offset;
            self.rope.append(chunk.text);
            offset += chunk.text.len();

            if !chunk.style.is_empty() {
                self.segments
                    .push(StyledSegment::new(start..offset, chunk.style));
            }
        }
    }

    /// Clear all content.
    pub fn clear(&mut self) {
        self.rope.clear();
        self.segments.clear();
        self.bump_revision();
    }

    /// Reset content and highlights.
    pub fn reset(&mut self) {
        self.clear();
    }

    /// Get the number of bytes.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Get the number of characters.
    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Get the number of lines.
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    /// Get a line by index.
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<String> {
        self.rope.line(idx).map(|s| s.to_string())
    }

    /// Iterate over all lines as owned strings.
    pub fn lines(&self) -> impl Iterator<Item = String> + '_ {
        self.rope.lines().map(|line| line.to_string())
    }

    /// Get the underlying rope.
    #[must_use]
    pub fn rope(&self) -> &RopeWrapper {
        &self.rope
    }

    /// Get mutable access to the rope.
    pub fn rope_mut(&mut self) -> &mut RopeWrapper {
        self.bump_revision();
        &mut self.rope
    }

    /// Get the buffer revision (increments on content changes).
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Add a highlight (styled segment) to a range.
    pub fn add_highlight(&mut self, range: Range<usize>, style: Style, priority: u8) {
        self.segments
            .push(StyledSegment::new(range, style).with_priority(priority));
    }

    /// Add a highlight by char range.
    pub fn add_highlight_by_char_range(
        &mut self,
        char_start: usize,
        char_end: usize,
        style: Style,
        priority: u8,
        ref_id: Option<u16>,
    ) {
        let start = self.rope.char_to_byte(char_start);
        let end = self.rope.char_to_byte(char_end);
        let mut segment = StyledSegment::new(start..end, style).with_priority(priority);
        let id = ref_id.unwrap_or(0);
        segment = segment.with_ref(id);
        self.segments.push(segment);
    }

    /// Add a highlight by line/column range.
    pub fn add_highlight_line(
        &mut self,
        line: usize,
        col_start: usize,
        col_end: usize,
        style: Style,
        priority: u8,
        ref_id: Option<u16>,
    ) {
        let Some(line_slice) = self.rope.line(line) else {
            return;
        };
        let line_len = line_slice.len_chars();
        let safe_start = col_start.min(line_len);
        let safe_end = col_end.min(line_len);

        if safe_start >= safe_end {
            return;
        }

        let line_start = self.rope.line_to_char(line);
        let start = self.rope.char_to_byte(line_start + safe_start);
        let end = self.rope.char_to_byte(line_start + safe_end);
        let mut segment = StyledSegment::new(start..end, style)
            .with_priority(priority)
            .with_line(line);
        let id = ref_id.unwrap_or(0);
        segment = segment.with_ref(id);
        self.segments.push(segment);
    }

    /// Add a highlight using a syntax style ID.
    pub fn add_highlight_with_style_id(
        &mut self,
        line: usize,
        col_start: usize,
        col_end: usize,
        style_id: u32,
        priority: u8,
        ref_id: Option<u16>,
    ) {
        let Some(registry) = self.syntax_styles.as_ref() else {
            return;
        };
        let Some(style) = registry.style(style_id) else {
            return;
        };
        self.add_highlight_line(line, col_start, col_end, style, priority, ref_id);
    }

    /// Clear all highlights.
    pub fn clear_highlights(&mut self) {
        self.segments
            .retain(|seg| seg.ref_id.is_none() && seg.line.is_none());
    }

    /// Remove all highlights with a specific reference ID.
    pub fn remove_highlights_by_ref(&mut self, ref_id: u16) {
        self.segments.retain(|seg| seg.ref_id != Some(ref_id));
    }

    /// Clear highlights for a specific line.
    pub fn clear_line_highlights(&mut self, line: usize) {
        self.segments.retain(|seg| seg.line != Some(line));
    }

    /// Clear highlights for a specific line and reference ID.
    pub fn clear_line_highlights_by_ref(&mut self, line: usize, ref_id: u16) {
        self.segments
            .retain(|seg| !(seg.line == Some(line) && seg.ref_id == Some(ref_id)));
    }

    /// Register external text in the memory registry.
    pub fn register_text(&mut self, text: &str, owned: bool) -> u32 {
        self.mem_registry.register(text, owned)
    }

    /// Replace external text by ID.
    pub fn replace_text_by_id(&mut self, id: u32, text: &str, owned: bool) {
        self.mem_registry.replace(id, text, owned);
    }

    /// Set buffer text from a registered memory ID.
    pub fn set_text_from_mem_id(&mut self, id: u32) {
        if let Some(text) = self.mem_registry.get(id).map(str::to_owned) {
            self.set_text(&text);
        }
    }

    /// Get segments overlapping a byte range.
    pub fn segments_in_range(&self, range: Range<usize>) -> impl Iterator<Item = &StyledSegment> {
        self.segments
            .iter()
            .filter(move |seg| seg.range.start < range.end && range.start < seg.range.end)
    }

    /// Get the style at a byte position.
    #[must_use]
    pub fn style_at(&self, pos: usize) -> Style {
        let mut style = self.default_style;
        let mut max_priority = 0u8;

        for seg in &self.segments {
            if seg.contains(pos) && seg.priority >= max_priority {
                style = style.merge(seg.style);
                max_priority = seg.priority;
            }
        }

        style
    }

    /// Convert to plain string.
    #[must_use]
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    #[test]
    fn test_buffer_basic() {
        let mut buffer = TextBuffer::new();
        buffer.set_text("Hello, world!");
        assert_eq!(buffer.len_chars(), 13);
    }

    #[test]
    fn test_buffer_styled_text() {
        let mut buffer = TextBuffer::new();
        buffer.set_styled_text(&[
            StyledChunk::new("Hello", Style::bold()),
            StyledChunk::plain(", "),
            StyledChunk::new("world", Style::fg(Rgba::RED)),
        ]);

        assert_eq!(buffer.to_string(), "Hello, world");
    }

    #[test]
    fn test_buffer_highlight() {
        let mut buffer = TextBuffer::new();
        buffer.set_text("Hello, world!");
        buffer.add_highlight(0..5, Style::bold(), 0);

        assert!(
            buffer
                .style_at(0)
                .attributes
                .contains(crate::style::TextAttributes::BOLD)
        );
        assert!(
            !buffer
                .style_at(6)
                .attributes
                .contains(crate::style::TextAttributes::BOLD)
        );
    }

    #[test]
    fn test_buffer_highlight_by_char_range_and_ref() {
        let mut buffer = TextBuffer::new();
        buffer.set_text("Hello, world!");
        buffer.add_highlight_by_char_range(7, 12, Style::underline(), 1, Some(42));
        assert!(
            buffer
                .style_at(buffer.rope().char_to_byte(8))
                .attributes
                .contains(crate::style::TextAttributes::UNDERLINE)
        );

        buffer.remove_highlights_by_ref(42);
        assert!(
            !buffer
                .style_at(buffer.rope().char_to_byte(8))
                .attributes
                .contains(crate::style::TextAttributes::UNDERLINE)
        );
    }

    #[test]
    fn test_mem_registry_set_text() {
        let mut buffer = TextBuffer::new();
        let id = buffer.register_text("External", true);
        buffer.set_text_from_mem_id(id);
        assert_eq!(buffer.to_string(), "External");
    }

    #[test]
    fn test_lines_iter() {
        let buffer = TextBuffer::with_text("Line 1\nLine 2");
        let lines: Vec<String> = buffer.lines().collect();
        assert_eq!(lines, vec!["Line 1\n".to_string(), "Line 2".to_string()]);
    }
}
