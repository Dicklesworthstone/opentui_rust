use std::sync::Once;

use tracing::{debug, info};

use crate::highlight::languages::python::PythonTokenizer;
use crate::highlight::{LineState, StringKind, TokenKind, Tokenizer};

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
fn test_python_keyword_recognition() {
    setup_test_logging();
    let tokenizer = PythonTokenizer::new();
    let keywords = ["def", "class", "import", "from", "return"];

    for kw in keywords {
        info!(keyword = kw, "testing keyword recognition");
        let (tokens, state) = tokenizer.tokenize_line(kw, LineState::Normal);
        debug!(?tokens, ?state, "tokenization result");
        assert!(!tokens.is_empty(), "expected tokens for keyword");
        assert!(matches!(
            tokens[0].kind,
            TokenKind::Keyword | TokenKind::KeywordControl
        ));
    }
}

#[test]
fn test_python_multiline_string_state() {
    setup_test_logging();
    let tokenizer = PythonTokenizer::new();
    let (tokens, state) = tokenizer.tokenize_line("text = \"\"\"start", LineState::Normal);
    debug!(?tokens, ?state, "triple string start");
    assert_eq!(state, LineState::InString(StringKind::Triple));

    let (tokens, state) = tokenizer.tokenize_line("middle", state);
    debug!(?tokens, ?state, "triple string middle");
    assert_eq!(state, LineState::InString(StringKind::Triple));

    let (tokens, state) = tokenizer.tokenize_line("end\"\"\"", state);
    debug!(?tokens, ?state, "triple string end");
    assert_eq!(state, LineState::Normal);
}

#[test]
fn test_python_number_formats() {
    setup_test_logging();
    let tokenizer = PythonTokenizer::new();
    let (tokens, _) = tokenizer.tokenize_line("0xFF 1_000 3.14", LineState::Normal);
    debug!(?tokens, "number formats");
    assert!(
        tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Number)
            .count()
            >= 3
    );
}
