//! Token types for syntax highlighting.

use std::ops::Range;

/// Semantic token categories used by tokenizers and themes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Keywords
    Keyword,
    KeywordControl,
    KeywordType,
    KeywordModifier,

    // Literals
    String,
    StringEscape,
    Number,
    Boolean,

    // Identifiers
    Identifier,
    Type,
    Constant,
    Function,
    Macro,

    // Comments
    Comment,
    CommentBlock,
    CommentDoc,

    // Operators and punctuation
    Operator,
    Punctuation,
    Delimiter,

    // Special
    Attribute,
    Lifetime,
    Label,

    // Markup (for markdown, etc.)
    Heading,
    Link,
    Emphasis,
    CodeInline,
    CodeBlock,

    // Errors
    Error,

    // Default
    Text,
}

/// A token produced by a tokenizer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

impl Token {
    #[must_use]
    pub fn new(kind: TokenKind, start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "token range must be start <= end");
        Self { kind, start, end }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

/// A token paired with its source text slice for rendering.
#[derive(Clone, Debug)]
pub struct TokenSpan<'a> {
    pub kind: TokenKind,
    pub text: &'a str,
}

#[cfg(test)]
mod tests {
    use super::{Token, TokenKind, TokenSpan};

    #[test]
    fn token_construction_and_accessors() {
        let sample = Token::new(TokenKind::Keyword, 2, 8);
        assert_eq!(sample.kind, TokenKind::Keyword);
        assert_eq!(sample.start, 2);
        assert_eq!(sample.end, 8);
        assert_eq!(sample.len(), 6);
        assert!(!sample.is_empty());
        assert_eq!(sample.range(), 2..8);
    }

    #[test]
    fn token_empty_range() {
        let sample = Token::new(TokenKind::Text, 5, 5);
        assert_eq!(sample.len(), 0);
        assert!(sample.is_empty());
    }

    #[test]
    fn token_kind_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<TokenKind>();
    }

    #[test]
    fn token_span_holds_slice() {
        let source = "let x = 1;";
        let span = TokenSpan {
            kind: TokenKind::Identifier,
            text: &source[4..5],
        };
        assert_eq!(span.text, "x");
        assert_eq!(span.kind, TokenKind::Identifier);
    }
}
