use crate::highlight::theme::Theme;
use crate::highlight::token::Token;
use crate::highlight::tokenizer::{LineState, Tokenizer};
use crate::style::Style;
use crate::text::{StyledSegment, TextBuffer};
use std::sync::Arc;

const SYNTAX_HIGHLIGHT_REF_ID: u16 = 1;

/// Text buffer with syntax highlighting support.
///
/// Wraps a [`TextBuffer`] and manages a tokenizer and theme to produce
/// styled text segments. Caches tokenization results per line for performance.
pub struct HighlightedBuffer {
    buffer: TextBuffer,
    tokenizer: Option<Arc<dyn Tokenizer>>,
    theme: Theme,

    // Per-line token cache
    line_tokens: Vec<Vec<Token>>,
    line_states: Vec<LineState>, // State at END of each line

    // Dirty tracking for incremental updates
    dirty_span: Option<std::ops::Range<usize>>,
    theme_dirty: bool,
}

impl HighlightedBuffer {
    /// Create a new highlighted buffer wrapping a text buffer.
    #[must_use]
    pub fn new(mut buffer: TextBuffer) -> Self {
        let theme = Theme::default();
        buffer.set_default_style(theme.default_style());
        let line_count = buffer.len_lines();

        Self {
            buffer,
            tokenizer: None,
            theme,
            line_tokens: vec![Vec::new(); line_count],
            line_states: vec![LineState::default(); line_count],
            dirty_span: Some(0..line_count),
            theme_dirty: false,
        }
    }

    /// Set the tokenizer (builder pattern).
    #[must_use]
    pub fn with_tokenizer(mut self, tokenizer: Box<dyn Tokenizer>) -> Self {
        self.set_tokenizer(Some(Arc::from(tokenizer)));
        self
    }

    /// Set the theme (builder pattern).
    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.set_theme(theme);
        self
    }

    /// Set the tokenizer. Triggers a full re-highlight on next update.
    pub fn set_tokenizer(&mut self, tokenizer: Option<Arc<dyn Tokenizer>>) {
        self.tokenizer = tokenizer;
        self.clear_syntax_highlights();
        let len = self.buffer.len_lines();
        self.mark_dirty(0, len);
        self.theme_dirty = true;
    }

    /// Set the theme. Does not require re-tokenization.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.buffer.set_default_style(self.theme.default_style());
        self.theme_dirty = true;
    }

    /// Get the current theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Returns true if a tokenizer is set.
    #[must_use]
    pub fn has_tokenizer(&self) -> bool {
        self.tokenizer.is_some()
    }

    /// Get the underlying text buffer.
    #[must_use]
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// Get mutable access to the underlying text buffer.
    ///
    /// **Note:** Modifications must be followed by `mark_dirty` if not done via
    /// `HighlightedBuffer` methods.
    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    /// Mark a range of lines as dirty.
    pub fn mark_dirty(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        if let Some(current) = &self.dirty_span {
            self.dirty_span = Some(current.start.min(start)..current.end.max(end));
        } else {
            self.dirty_span = Some(start..end);
        }
    }

    /// Re-tokenize dirty lines and update highlight segments.
    ///
    /// Should be called before rendering if the buffer has changed.
    pub fn update_highlighting(&mut self) {
        let Some(tokenizer) = self.tokenizer.clone() else {
            return;
        };

        let buffer = &mut self.buffer;
        let line_count = buffer.len_lines();

        let line_tokens = &mut self.line_tokens;
        let line_states = &mut self.line_states;
        let count_changed = line_count != line_tokens.len();

        if count_changed {
            line_tokens.resize(line_count, Vec::new());
            line_states.resize(line_count, LineState::default());
            // Full re-tokenize if line count changed
            self.dirty_span = Some(0..line_count);
        }

        let retokenize = self.dirty_span.is_some();
        let mut start_line = if retokenize {
            self.dirty_span.as_ref().unwrap().start
        } else if self.theme_dirty {
            0
        } else {
            return;
        };

        // Clamp start line
        start_line = start_line.min(line_count);

        // If theme changed, we need to re-apply styles even if tokens are valid
        // But for simplicity, we treat theme dirty as a full re-process pass logic
        // reusing tokens if not dirty.
        // Actually, if theme changed, we might not need to re-tokenize, just re-apply highlights.
        // But apply_line_highlights is called inside the loop.
        // So we can just iterate.

        if retokenize || self.theme_dirty {
            // Determine end of mandatory processing
            let mandatory_end = if retokenize {
                self.dirty_span.as_ref().unwrap().end.min(line_count)
            } else {
                0
            };

            // If theme dirty, we must process EVERYTHING to update styles?
            // Yes, apply_line_highlights uses the theme.
            // So if theme dirty, treat as if dirty_span is full?
            // Or just iterate all and skip tokenize if not dirty?
            // To keep logic simple: if theme dirty, we iterate 0..line_count.

            let (loop_start, loop_end) = if self.theme_dirty {
                (0, line_count)
            } else {
                (start_line, line_count)
            };

            let mut state = if loop_start > 0 {
                line_states[loop_start - 1]
            } else {
                LineState::Normal
            };

            for i in loop_start..loop_end {
                // Check if this line is in the dirty span
                let in_dirty_span = self
                    .dirty_span
                    .as_ref()
                    .is_some_and(|span| i >= span.start && i < span.end);

                let new_state;

                if in_dirty_span || self.theme_dirty {
                    // Early exit: if past mandatory range, state unchanged, and theme clean
                    if i >= mandatory_end && state == line_states[i] && !self.theme_dirty {
                        break;
                    }

                    // Tokenize if in dirty span or state changed
                    if i < mandatory_end || state != line_states[i] {
                        let Some(line_str) = buffer.line(i) else {
                            break;
                        };
                        let line_content = line_str.trim_end_matches(['\n', '\r']);
                        let (tokens, ns) = tokenizer.tokenize_line(line_content, state);
                        new_state = ns;
                        if line_tokens[i] != tokens {
                            line_tokens[i] = tokens;
                        }
                    } else {
                        // Re-using cached tokens (state matched, not dirty content)
                        new_state = line_states[i];
                    }

                    if line_states[i] != new_state {
                        line_states[i] = new_state;
                    }
                } else {
                    // Not in dirty span, theme not dirty.
                    // Why are we here?
                    // Because loop_end is line_count.
                    // logic above handles breaking.
                    break;
                }

                // Always apply highlights if we are here (dirty, state change, or theme change)
                Self::apply_line_highlights(buffer, &self.theme, i, &line_tokens[i]);
                state = new_state;
            }
        }

        self.theme_dirty = false;
        self.dirty_span = None;
    }

    /// Get tokens for a line.
    #[must_use]
    pub fn tokens_for_line(&self, line: usize) -> &[Token] {
        self.line_tokens.get(line).map_or(&[], Vec::as_slice)
    }

    /// Get styled segments for a line, merging highlighting with existing styles.
    #[must_use]
    pub fn styled_line(&self, line: usize) -> Vec<StyledSegment> {
        let mut segments = Vec::new();
        let Some(line_str) = self.buffer.line(line) else {
            return segments;
        };

        let line_start = self.buffer.rope().line_to_char(line);
        let line_start_byte = self.buffer.rope().char_to_byte(line_start);
        let line_byte_len = line_str.len();

        if let Some(tokens) = self.line_tokens.get(line) {
            for token in tokens {
                // Validate token bounds: skip malformed tokens
                if token.start > token.end || token.end > line_byte_len {
                    continue;
                }

                let style = self.theme.style_for(token.kind);
                if *style != Style::default() {
                    let start = line_start_byte + token.start;
                    let end = line_start_byte + token.end;
                    segments.push(StyledSegment::new(start..end, *style));
                }
            }
        }

        segments
    }

    /// Get the underlying rope.
    #[must_use]
    pub fn rope(&self) -> &crate::text::RopeWrapper {
        self.buffer.rope()
    }

    /// Get mutable access to the rope.
    ///
    /// **Note:** Caller must call `mark_dirty` after modifications!
    pub fn rope_mut(&mut self) -> &mut crate::text::RopeWrapper {
        self.buffer.rope_mut()
    }

    /// Get the number of characters.
    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.buffer.len_chars()
    }

    /// Get the number of lines.
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.buffer.len_lines()
    }

    /// Get a line by index.
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<String> {
        self.buffer.line(idx)
    }

    /// Convert to string.
    #[must_use]
    pub fn to_string(&self) -> String {
        self.buffer.to_string()
    }

    /// Set the text content.
    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(text);
        let line_count = self.buffer.len_lines();
        self.line_tokens.clear();
        self.line_tokens.resize(line_count, Vec::new());
        self.line_states.clear();
        self.line_states.resize(line_count, LineState::default());
        self.dirty_span = Some(0..line_count);
    }

    fn clear_syntax_highlights(&mut self) {
        self.buffer
            .remove_highlights_by_ref(SYNTAX_HIGHLIGHT_REF_ID);
    }

    fn apply_line_highlights(
        buffer: &mut TextBuffer,
        theme: &Theme,
        line: usize,
        tokens: &[Token],
    ) {
        buffer.clear_line_highlights_by_ref(line, SYNTAX_HIGHLIGHT_REF_ID);

        let line_start_char = buffer.rope().line_to_char(line);
        let line_start_byte = buffer.rope().char_to_byte(line_start_char);

        // Calculate line byte length for bounds validation
        let line_end_char = if line + 1 < buffer.rope().len_lines() {
            buffer.rope().line_to_char(line + 1)
        } else {
            buffer.rope().len_chars()
        };
        let line_end_byte = buffer.rope().char_to_byte(line_end_char);
        let line_byte_len = line_end_byte.saturating_sub(line_start_byte);

        for token in tokens {
            let style = theme.style_for(token.kind);
            if *style == Style::default() {
                continue;
            }

            // Validate token bounds: skip malformed tokens
            if token.start > token.end || token.end > line_byte_len {
                continue;
            }

            let start_byte = line_start_byte + token.start;
            let end_byte = line_start_byte + token.end;
            let start_char = buffer.rope().byte_to_char(start_byte);
            let end_char = buffer.rope().byte_to_char(end_byte);
            let col_start = start_char.saturating_sub(line_start_char);
            let col_end = end_char.saturating_sub(line_start_char);

            if col_start >= col_end {
                continue;
            }

            buffer.add_highlight_line(
                line,
                col_start,
                col_end,
                *style,
                0,
                Some(SYNTAX_HIGHLIGHT_REF_ID),
            );
        }
    }
}

impl Default for HighlightedBuffer {
    fn default() -> Self {
        Self::new(TextBuffer::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::languages::rust::RustTokenizer;
    use crate::highlight::token::TokenKind;

    #[test]
    fn test_highlighted_buffer_basic() {
        let mut buffer = HighlightedBuffer::new(TextBuffer::with_text("fn main() {}"));
        buffer.set_tokenizer(Some(Arc::new(RustTokenizer::new())));
        buffer.update_highlighting();

        let tokens = buffer.tokens_for_line(0);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Keyword));
    }

    #[test]
    fn test_theme_change_updates_styles() {
        let mut buffer = HighlightedBuffer::new(TextBuffer::with_text("fn main() {}"));
        buffer.set_tokenizer(Some(Arc::new(RustTokenizer::new())));
        buffer.update_highlighting();

        let line_start = buffer.buffer().rope().line_to_char(0);
        let start_byte = buffer.buffer().rope().char_to_byte(line_start);
        let keyword_style = buffer.buffer().style_at(start_byte);

        let new_theme = Theme::light();
        buffer.set_theme(new_theme.clone());
        buffer.update_highlighting();

        let updated_style = buffer.buffer().style_at(start_byte);
        assert_ne!(keyword_style, updated_style);
        let expected = buffer
            .buffer()
            .default_style()
            .merge(*new_theme.style_for(TokenKind::Keyword));
        assert_eq!(updated_style, expected);
    }

    #[test]
    fn test_incremental_update_single_line() {
        let mut buffer = HighlightedBuffer::new(TextBuffer::with_text("let a = 1;\nlet b = 2;"));
        buffer.set_tokenizer(Some(Arc::new(RustTokenizer::new())));
        buffer.update_highlighting();
        let tokens_before = buffer.tokens_for_line(1).to_vec();

        buffer.buffer_mut().rope_mut().insert(0, "const ");
        buffer.mark_dirty(0, 1);
        buffer.update_highlighting();

        let tokens_after = buffer.tokens_for_line(1).to_vec();
        assert_eq!(tokens_before, tokens_after);
    }

    /// Mock tokenizer that produces malformed tokens for testing bounds validation.
    struct MalformedTokenizer;

    impl crate::highlight::tokenizer::Tokenizer for MalformedTokenizer {
        fn name(&self) -> &'static str {
            "malformed-test"
        }

        fn extensions(&self) -> &'static [&'static str] {
            &[]
        }

        fn tokenize_line(&self, _line: &str, state: LineState) -> (Vec<Token>, LineState) {
            // Produce tokens with various invalid bounds:
            // - Token with start > end (inverted)
            // - Token with end exceeding line length
            // - Token at line boundary (valid)
            let tokens = vec![
                Token {
                    kind: TokenKind::Keyword,
                    start: 10,
                    end: 5,
                }, // Inverted
                Token {
                    kind: TokenKind::String,
                    start: 0,
                    end: 1000,
                }, // Exceeds line
                Token {
                    kind: TokenKind::Comment,
                    start: 0,
                    end: 2,
                }, // Valid
            ];
            (tokens, state)
        }
    }

    #[test]
    fn test_malformed_token_bounds_are_skipped() {
        // This test verifies that tokens with invalid bounds don't cause panics
        let mut buffer = HighlightedBuffer::new(TextBuffer::with_text("hello"));
        buffer.set_tokenizer(Some(Arc::new(MalformedTokenizer)));

        // Should not panic even with malformed tokens
        buffer.update_highlighting();

        // styled_line should also handle malformed tokens gracefully
        let segments = buffer.styled_line(0);

        // Only the valid token (Comment, 0..2) should produce a segment
        // (assuming Comment has a non-default style in the theme)
        assert!(
            segments.len() <= 1,
            "Only valid tokens should produce segments"
        );
    }
}
