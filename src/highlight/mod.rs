//! Syntax highlighting and style management.

pub mod highlighted_buffer;
pub mod languages;
mod syntax;
pub mod theme;
pub mod token;
pub mod tokenizer;

pub use highlighted_buffer::HighlightedBuffer;
pub use syntax::{SyntaxStyle, SyntaxStyleRegistry};
pub use theme::{Theme, ThemeRegistry};
pub use token::{Token, TokenKind, TokenSpan};
pub use tokenizer::{
    CommentKind, HeredocKind, LineState, StringKind, Tokenizer, TokenizerRegistry,
};

#[cfg(test)]
mod tests;
