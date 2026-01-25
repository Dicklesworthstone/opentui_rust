use std::sync::Once;

use tracing::{debug, info};

use crate::highlight::languages::rust::RustTokenizer;
use crate::highlight::{CommentKind, LineState, TokenKind, Tokenizer};

fn setup_test_logging() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();
    });
}

#[test]
fn test_rust_keyword_recognition() {
    setup_test_logging();
    let tokenizer = RustTokenizer::new();
    let keywords = ["fn", "let", "mut", "if", "else", "match"];

    for kw in keywords {
        info!(keyword = kw, "testing keyword recognition");
        let (tokens, state) = tokenizer.tokenize_line(kw, LineState::Normal);
        debug!(?tokens, ?state, "tokenization result");
        assert!(!tokens.is_empty(), "expected tokens for keyword");
        assert!(matches!(
            tokens[0].kind,
            TokenKind::Keyword | TokenKind::KeywordControl | TokenKind::KeywordModifier
        ));
    }
}

#[test]
fn test_rust_multiline_block_comment_state() {
    setup_test_logging();
    let tokenizer = RustTokenizer::new();
    let (tokens, state) = tokenizer.tokenize_line("/* block", LineState::Normal);
    debug!(?tokens, ?state, "block comment start");
    assert_eq!(state, LineState::InComment(CommentKind::Block));

    let (tokens, state) = tokenizer.tokenize_line("end */", state);
    debug!(?tokens, ?state, "block comment end");
    assert_eq!(state, LineState::Normal);
}

#[test]
fn test_rust_lifetime_vs_char_literal() {
    setup_test_logging();
    let tokenizer = RustTokenizer::new();
    let (tokens, _) = tokenizer.tokenize_line("'a' 'a", LineState::Normal);
    debug!(?tokens, "lifetime vs char literal");
    assert_eq!(tokens[0].kind, TokenKind::String);
    assert_eq!(tokens[1].kind, TokenKind::Lifetime);
}
