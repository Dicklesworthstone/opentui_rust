//! Syntax highlighting and style management.

mod syntax;
pub mod token;
pub mod tokenizer;

pub use syntax::{SyntaxStyle, SyntaxStyleRegistry};
pub use token::{Token, TokenKind, TokenSpan};
pub use tokenizer::{
    CommentKind, HeredocKind, LineState, StringKind, Tokenizer, TokenizerRegistry,
};
