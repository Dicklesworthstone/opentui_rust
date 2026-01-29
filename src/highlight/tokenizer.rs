//! Tokenizer traits and line state for syntax highlighting.

use std::collections::HashMap;
use std::sync::Arc;

use super::token::Token;

/// Lexical state carried across lines for incremental tokenization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum LineState {
    #[default]
    Normal,
    InString(StringKind),
    InComment(CommentKind),
    InRawString(u8),
    InHeredoc(HeredocKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StringKind {
    Double,
    Single,
    Backtick,
    Triple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommentKind {
    Block,
    Doc,
    Nested(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeredocKind {
    Shell,
    Ruby,
}

/// Core tokenizer abstraction for syntax highlighting.
pub trait Tokenizer: Send + Sync {
    /// Human-readable name of this tokenizer.
    fn name(&self) -> &'static str;

    /// File extensions this tokenizer handles (e.g., `rs`, `rust`).
    fn extensions(&self) -> &'static [&'static str];

    /// Tokenize a single line given the state from the previous line.
    /// Returns: (tokens, state_at_end_of_line).
    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState);

    /// Tokenize an entire text by calling `tokenize_line` for each line.
    ///
    /// Handles both LF (`\n`) and CRLF (`\r\n`) line endings correctly by
    /// tracking the actual byte position in the original text rather than
    /// assuming a fixed line separator length.
    fn tokenize(&self, text: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut state = LineState::Normal;
        let mut offset = 0usize;
        let bytes = text.as_bytes();

        for line in text.lines() {
            let (line_tokens, new_state) = self.tokenize_line(line, state);
            for mut token in line_tokens {
                token.start += offset;
                token.end += offset;
                tokens.push(token);
            }

            // Advance offset past the line content
            offset += line.len();

            // Advance past the line ending (LF, CRLF, or end of text)
            // Check for CRLF first, then LF
            if offset < bytes.len() {
                if bytes[offset] == b'\r' && offset + 1 < bytes.len() && bytes[offset + 1] == b'\n'
                {
                    offset += 2; // CRLF
                } else if bytes[offset] == b'\n' {
                    offset += 1; // LF
                } else if bytes[offset] == b'\r' {
                    offset += 1; // Bare CR (old Mac style)
                }
            }

            state = new_state;
        }

        tokens
    }
}

/// Registry for tokenizer lookup by extension or name.
#[derive(Default)]
pub struct TokenizerRegistry {
    tokenizers: Vec<Arc<dyn Tokenizer>>,
    by_extension: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
}

impl TokenizerRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tokenizer. Later registrations override existing lookups.
    pub fn register(&mut self, tokenizer: Box<dyn Tokenizer>) {
        let tokenizer: Arc<dyn Tokenizer> = Arc::from(tokenizer);
        let index = self.tokenizers.len();
        let name_key = tokenizer.name().to_ascii_lowercase();
        self.by_name.insert(name_key, index);

        for ext in tokenizer.extensions() {
            let key = ext.trim_start_matches('.').to_ascii_lowercase();
            if !key.is_empty() {
                self.by_extension.insert(key, index);
            }
        }

        self.tokenizers.push(tokenizer);
    }

    /// Get tokenizer by file extension (case-insensitive, with or without dot).
    #[must_use]
    pub fn for_extension(&self, ext: &str) -> Option<&dyn Tokenizer> {
        let key = ext.trim_start_matches('.').to_ascii_lowercase();
        let index = self.by_extension.get(&key)?;
        self.tokenizers.get(*index).map(AsRef::as_ref)
    }

    /// Get tokenizer by file extension (case-insensitive, with or without dot).
    #[must_use]
    pub fn for_extension_shared(&self, ext: &str) -> Option<Arc<dyn Tokenizer>> {
        let key = ext.trim_start_matches('.').to_ascii_lowercase();
        let index = self.by_extension.get(&key)?;
        self.tokenizers.get(*index).cloned()
    }

    /// Get tokenizer by name (case-insensitive).
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&dyn Tokenizer> {
        let key = name.to_ascii_lowercase();
        let index = self.by_name.get(&key)?;
        self.tokenizers.get(*index).map(AsRef::as_ref)
    }

    /// Get tokenizer by name (case-insensitive).
    #[must_use]
    pub fn by_name_shared(&self, name: &str) -> Option<Arc<dyn Tokenizer>> {
        let key = name.to_ascii_lowercase();
        let index = self.by_name.get(&key)?;
        self.tokenizers.get(*index).cloned()
    }

    /// Create registry with all built-in tokenizers.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(
            crate::highlight::languages::javascript::JavaScriptTokenizer::javascript(),
        ));
        registry.register(Box::new(
            crate::highlight::languages::javascript::JavaScriptTokenizer::typescript(),
        ));
        registry.register(Box::new(
            crate::highlight::languages::json::JsonTokenizer::new(),
        ));
        registry.register(Box::new(
            crate::highlight::languages::markdown::MarkdownTokenizer::new(),
        ));
        registry.register(Box::new(
            crate::highlight::languages::python::PythonTokenizer::new(),
        ));
        registry.register(Box::new(
            crate::highlight::languages::rust::RustTokenizer::new(),
        ));
        registry.register(Box::new(
            crate::highlight::languages::toml::TomlTokenizer::new(),
        ));
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::{CommentKind, HeredocKind, LineState, StringKind, Tokenizer, TokenizerRegistry};
    use crate::highlight::{Token, TokenKind};

    struct StubTokenizer;

    impl Tokenizer for StubTokenizer {
        fn name(&self) -> &'static str {
            "Stub"
        }

        fn extensions(&self) -> &'static [&'static str] {
            &["rs", "RUST"]
        }

        fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
            let span = Token::new(TokenKind::Text, 0, line.len());
            (vec![span], state)
        }
    }

    #[test]
    fn line_state_default_is_normal() {
        assert_eq!(LineState::default(), LineState::Normal);
        let _ = LineState::InString(StringKind::Double);
        let _ = LineState::InComment(CommentKind::Block);
        let _ = LineState::InRawString(2);
        let _ = LineState::InHeredoc(HeredocKind::Shell);
    }

    #[test]
    fn tokenizer_default_tokenize_offsets_lines_lf() {
        let tokenizer = StubTokenizer;
        // LF line endings: "aa\nbbb" = 6 bytes total
        let tokens = tokenizer.tokenize("aa\nbbb");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].range(), 0..2); // "aa" at bytes 0-2
        assert_eq!(tokens[1].range(), 3..6); // "bbb" at bytes 3-6
    }

    #[test]
    fn tokenizer_default_tokenize_offsets_lines_crlf() {
        let tokenizer = StubTokenizer;
        // CRLF line endings: "aa\r\nbbb" = 7 bytes total
        // Line 1: "aa" at bytes 0-2, then \r\n at bytes 2-4
        // Line 2: "bbb" at bytes 4-7
        let tokens = tokenizer.tokenize("aa\r\nbbb");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].range(), 0..2); // "aa" at bytes 0-2
        assert_eq!(tokens[1].range(), 4..7); // "bbb" at bytes 4-7 (after \r\n)
    }

    #[test]
    fn tokenizer_default_tokenize_offsets_lines_mixed() {
        let tokenizer = StubTokenizer;
        // Mixed line endings: "a\nb\r\nc" = 7 bytes total
        // Line 1: "a" at byte 0, then \n at byte 1
        // Line 2: "b" at byte 2, then \r\n at bytes 3-5
        // Line 3: "c" at byte 5
        let tokens = tokenizer.tokenize("a\nb\r\nc");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].range(), 0..1); // "a" at byte 0
        assert_eq!(tokens[1].range(), 2..3); // "b" at byte 2
        assert_eq!(tokens[2].range(), 5..6); // "c" at byte 5
    }

    #[test]
    fn tokenizer_handles_trailing_newline() {
        let tokenizer = StubTokenizer;
        // Trailing LF: "aa\n" = 3 bytes, single line
        let tokens = tokenizer.tokenize("aa\n");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].range(), 0..2);

        // Trailing CRLF: "aa\r\n" = 4 bytes, single line
        let tokens = tokenizer.tokenize("aa\r\n");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].range(), 0..2);
    }

    #[test]
    fn tokenizer_handles_empty_lines_crlf() {
        let tokenizer = StubTokenizer;
        // Empty line with CRLF: "a\r\n\r\nb" = 6 bytes
        // Line 1: "a" at byte 0, then \r\n at bytes 1-3
        // Line 2: "" (empty), then \r\n at bytes 3-5
        // Line 3: "b" at byte 5
        let tokens = tokenizer.tokenize("a\r\n\r\nb");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].range(), 0..1); // "a"
        assert_eq!(tokens[1].range(), 3..3); // empty line
        assert_eq!(tokens[2].range(), 5..6); // "b"
    }

    #[test]
    fn registry_lookup_by_extension_and_name() {
        let mut registry = TokenizerRegistry::new();
        registry.register(Box::new(StubTokenizer));

        assert!(registry.for_extension("rs").is_some());
        assert!(registry.for_extension(".RS").is_some());
        assert!(registry.for_extension_shared("rs").is_some());
        assert!(registry.by_name("stub").is_some());
        assert!(registry.by_name("STUB").is_some());
        assert!(registry.by_name_shared("stub").is_some());
        assert!(registry.by_name("missing").is_none());
    }

    #[test]
    fn tokenizer_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StubTokenizer>();
    }
}
