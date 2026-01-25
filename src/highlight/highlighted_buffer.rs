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
    dirty_from: Option<usize>, // First dirty line
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
            dirty_from: Some(0),
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
        self.mark_dirty(0);
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
    /// `HighlightedBuffer` methods (which don't exist yet, so you must assume
    /// manual dirty marking is required if you touch the inner buffer).
    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        // Assume any access might dirty the buffer.
        // Ideally we'd wrap all mutation methods, but for now we expose it.
        // User must call mark_dirty!
        &mut self.buffer
    }

    /// Mark lines as dirty starting from a specific line.
    pub fn mark_dirty(&mut self, from_line: usize) {
        if let Some(current) = self.dirty_from {
            self.dirty_from = Some(current.min(from_line));
        } else {
            self.dirty_from = Some(from_line);
        }
    }

    /// Re-tokenize dirty lines and update highlight segments.
    ///
    /// Should be called before rendering if the buffer has changed.
    pub fn update_highlighting(&mut self) {
        let Some(tokenizer) = self.tokenizer.clone() else {
            return;
        };

        let mut retokenize = self.dirty_from.is_some();
        let mut start_line = if retokenize {
            self.dirty_from.take().unwrap_or(0)
        } else if self.theme_dirty {
            0
        } else {
            return;
        };

        let buffer = &mut self.buffer;
        let theme = &self.theme;
        let line_tokens = &mut self.line_tokens;
        let line_states = &mut self.line_states;

        let line_count = buffer.len_lines();
        let count_changed = line_count != line_tokens.len();
        line_tokens.resize(line_count, Vec::new());
        line_states.resize(line_count, LineState::default());
        if count_changed {
            retokenize = true;
            start_line = 0;
        }

        if retokenize {
            let mut state = if start_line > 0 {
                line_states[start_line - 1]
            } else {
                LineState::Normal
            };

            for i in start_line..line_count {
                let Some(line_str) = buffer.line(i) else {
                    break;
                };
                let line_content = line_str.trim_end_matches(['\n', '\r']);

                let (tokens, new_state) = tokenizer.tokenize_line(line_content, state);
                let tokens_changed = line_tokens[i] != tokens;
                let state_changed = line_states[i] != new_state;

                if tokens_changed {
                    line_tokens[i] = tokens;
                }
                if state_changed {
                    line_states[i] = new_state;
                }

                Self::apply_line_highlights(buffer, theme, i, &line_tokens[i]);
                state = new_state;

                if !self.theme_dirty && !tokens_changed && !state_changed {
                    break;
                }
            }
        } else {
            for (i, tokens) in line_tokens.iter().enumerate() {
                Self::apply_line_highlights(buffer, theme, i, tokens);
            }
        }

        self.theme_dirty = false;
        self.dirty_from = None;
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
        let Some(_line_str) = self.buffer.line(line) else {
            return segments;
        };

        let line_start = self.buffer.rope().line_to_char(line);
        let line_start_byte = self.buffer.rope().char_to_byte(line_start);

        if let Some(tokens) = self.line_tokens.get(line) {
            for token in tokens {
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
        self.dirty_from = Some(0);
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

        for token in tokens {
            let style = theme.style_for(token.kind);
            if *style == Style::default() {
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
        buffer.mark_dirty(0);
        buffer.update_highlighting();

        let tokens_after = buffer.tokens_for_line(1).to_vec();
        assert_eq!(tokens_before, tokens_after);
    }
}
