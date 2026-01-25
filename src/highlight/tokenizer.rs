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
    fn tokenize(&self, text: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut state = LineState::Normal;
        let mut offset = 0usize;

        for line in text.lines() {
            let (line_tokens, new_state) = self.tokenize_line(line, state);
            for mut token in line_tokens {
                token.start += offset;
                token.end += offset;
                tokens.push(token);
            }
            offset += line.len() + 1;
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
    fn tokenizer_default_tokenize_offsets_lines() {
        let tokenizer = StubTokenizer;
        let tokens = tokenizer.tokenize("aa\nbbb");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].range(), 0..2);
        assert_eq!(tokens[1].range(), 3..6);
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
