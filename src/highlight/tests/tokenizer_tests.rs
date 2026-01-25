use std::sync::Once;

use tracing::{debug, info};

use crate::highlight::languages::javascript::JavaScriptTokenizer;
use crate::highlight::languages::json::JsonTokenizer;
use crate::highlight::languages::markdown::MarkdownTokenizer;
use crate::highlight::languages::python::PythonTokenizer;
use crate::highlight::languages::rust::RustTokenizer;
use crate::highlight::languages::toml::TomlTokenizer;
use crate::highlight::{LineState, StringKind, Token, TokenKind, Tokenizer};

fn setup_test_logging() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();
    });
}

fn assert_non_overlapping(tokens: &[Token], line_len: usize) {
    let mut last_end = 0usize;
    for token in tokens {
        assert!(token.start <= token.end, "token has invalid range");
        assert!(token.end <= line_len, "token exceeds line length");
        assert!(token.start >= last_end, "token overlaps previous token");
        last_end = token.end;
    }
}

macro_rules! assert_first_kind {
    ($name:ident, $tokenizer:expr, $input:expr, $kind:expr) => {
        #[test]
        fn $name() {
            setup_test_logging();
            let tokenizer = $tokenizer;
            info!(case = $input, "tokenizing");
            let (tokens, state) = tokenizer.tokenize_line($input, LineState::Normal);
            debug!(?tokens, ?state, "tokenization result");
            assert!(!tokens.is_empty(), "tokens should not be empty");
            assert_eq!(tokens[0].kind, $kind);
            assert_non_overlapping(&tokens, $input.len());
        }
    };
}

macro_rules! assert_any_kind {
    ($name:ident, $tokenizer:expr, $input:expr, $kind:expr) => {
        #[test]
        fn $name() {
            setup_test_logging();
            let tokenizer = $tokenizer;
            info!(case = $input, "tokenizing");
            let (tokens, state) = tokenizer.tokenize_line($input, LineState::Normal);
            debug!(?tokens, ?state, "tokenization result");
            assert!(tokens.iter().any(|t| t.kind == $kind));
        }
    };
}

assert_first_kind!(
    rust_keyword_fn,
    RustTokenizer::new(),
    "fn",
    TokenKind::Keyword
);
assert_first_kind!(
    rust_keyword_if,
    RustTokenizer::new(),
    "if",
    TokenKind::KeywordControl
);
assert_first_kind!(
    rust_boolean_true,
    RustTokenizer::new(),
    "true",
    TokenKind::Boolean
);
assert_first_kind!(
    rust_type_u8,
    RustTokenizer::new(),
    "u8",
    TokenKind::KeywordType
);
assert_any_kind!(
    rust_macro_call,
    RustTokenizer::new(),
    "println!",
    TokenKind::Macro
);
assert_any_kind!(
    rust_attribute,
    RustTokenizer::new(),
    "#[test]",
    TokenKind::Attribute
);
assert_any_kind!(
    rust_lifetime,
    RustTokenizer::new(),
    "'a",
    TokenKind::Lifetime
);
assert_any_kind!(
    rust_label,
    RustTokenizer::new(),
    "'label: loop {}",
    TokenKind::Label
);
assert_any_kind!(
    rust_comment_block,
    RustTokenizer::new(),
    "/* block */",
    TokenKind::CommentBlock
);
assert_any_kind!(
    rust_comment_doc,
    RustTokenizer::new(),
    "/** doc */",
    TokenKind::CommentDoc
);

assert_first_kind!(
    python_keyword_def,
    PythonTokenizer::new(),
    "def",
    TokenKind::Keyword
);
assert_first_kind!(
    python_keyword_if,
    PythonTokenizer::new(),
    "if",
    TokenKind::KeywordControl
);
assert_first_kind!(
    python_boolean_true,
    PythonTokenizer::new(),
    "True",
    TokenKind::Boolean
);
assert_first_kind!(
    python_none_keyword,
    PythonTokenizer::new(),
    "None",
    TokenKind::Keyword
);
assert_any_kind!(
    python_decorator,
    PythonTokenizer::new(),
    "@decorator",
    TokenKind::Attribute
);
assert_any_kind!(
    python_string,
    PythonTokenizer::new(),
    "\"hello\"",
    TokenKind::String
);
assert_any_kind!(
    python_number,
    PythonTokenizer::new(),
    "123",
    TokenKind::Number
);
assert_any_kind!(
    python_function_call,
    PythonTokenizer::new(),
    "call()",
    TokenKind::Function
);

assert_first_kind!(
    js_keyword_if,
    JavaScriptTokenizer::javascript(),
    "if",
    TokenKind::KeywordControl
);
assert_first_kind!(
    js_keyword_function,
    JavaScriptTokenizer::javascript(),
    "function",
    TokenKind::Keyword
);
assert_any_kind!(
    js_boolean,
    JavaScriptTokenizer::javascript(),
    "true",
    TokenKind::Boolean
);
assert_any_kind!(
    js_constant_null,
    JavaScriptTokenizer::javascript(),
    "null",
    TokenKind::Constant
);
assert_any_kind!(
    js_operator,
    JavaScriptTokenizer::javascript(),
    "a + b",
    TokenKind::Operator
);
assert_any_kind!(
    js_template_string,
    JavaScriptTokenizer::javascript(),
    "`hi`",
    TokenKind::String
);
assert_any_kind!(
    js_regex_literal,
    JavaScriptTokenizer::javascript(),
    "/ab+/g",
    TokenKind::String
);
assert_any_kind!(
    js_ts_interface,
    JavaScriptTokenizer::typescript(),
    "interface",
    TokenKind::KeywordType
);

assert_any_kind!(
    json_key,
    JsonTokenizer::new(),
    "\"key\": 1",
    TokenKind::Identifier
);
assert_any_kind!(
    json_string,
    JsonTokenizer::new(),
    "\"value\"",
    TokenKind::String
);
assert_any_kind!(json_number, JsonTokenizer::new(), "3.14", TokenKind::Number);
assert_any_kind!(
    json_boolean,
    JsonTokenizer::new(),
    "true",
    TokenKind::Boolean
);
assert_any_kind!(json_null, JsonTokenizer::new(), "null", TokenKind::Constant);
assert_any_kind!(
    json_delimiter,
    JsonTokenizer::new(),
    "1,2",
    TokenKind::Delimiter
);

assert_any_kind!(
    toml_section,
    TomlTokenizer::new(),
    "[section]",
    TokenKind::Type
);
assert_any_kind!(
    toml_key,
    TomlTokenizer::new(),
    "key = 1",
    TokenKind::Identifier
);
assert_any_kind!(
    toml_string,
    TomlTokenizer::new(),
    "name = \"value\"",
    TokenKind::String
);
assert_any_kind!(
    toml_number,
    TomlTokenizer::new(),
    "num = 42",
    TokenKind::Number
);
assert_any_kind!(
    toml_boolean,
    TomlTokenizer::new(),
    "flag = true",
    TokenKind::Boolean
);
assert_any_kind!(
    toml_operator,
    TomlTokenizer::new(),
    "key = 1",
    TokenKind::Operator
);

assert_any_kind!(
    md_heading,
    MarkdownTokenizer::new(),
    "# Heading",
    TokenKind::Heading
);
assert_any_kind!(
    md_emphasis,
    MarkdownTokenizer::new(),
    "*italic*",
    TokenKind::Emphasis
);
assert_any_kind!(
    md_code_inline,
    MarkdownTokenizer::new(),
    "`code`",
    TokenKind::CodeInline
);
assert_any_kind!(
    md_link,
    MarkdownTokenizer::new(),
    "[link](url)",
    TokenKind::Link
);
assert_any_kind!(
    md_list,
    MarkdownTokenizer::new(),
    "- item",
    TokenKind::Punctuation
);
assert_any_kind!(
    md_blockquote,
    MarkdownTokenizer::new(),
    "> quote",
    TokenKind::Comment
);
assert_any_kind!(
    md_code_block,
    MarkdownTokenizer::new(),
    "```",
    TokenKind::CodeBlock
);

#[test]
fn test_empty_and_whitespace() {
    setup_test_logging();
    info!("testing empty and whitespace inputs");
    let tokenizer = JsonTokenizer::new();
    let (tokens, state) = tokenizer.tokenize_line("", LineState::Normal);
    debug!(?tokens, ?state, "empty line");
    assert!(tokens.is_empty());

    let (tokens, state) = tokenizer.tokenize_line("   ", LineState::Normal);
    debug!(?tokens, ?state, "whitespace line");
    assert!(tokens.is_empty());
}

#[test]
fn test_rust_block_comment_state() {
    setup_test_logging();
    info!("testing rust block comment state");
    let tokenizer = RustTokenizer::new();
    let (tokens, state) = tokenizer.tokenize_line("/* block", LineState::Normal);
    debug!(?tokens, ?state, "block comment start");
    assert_eq!(
        state,
        LineState::InComment(crate::highlight::CommentKind::Block)
    );
}

#[test]
fn test_markdown_fenced_state() {
    setup_test_logging();
    info!("testing markdown fenced state");
    let tokenizer = MarkdownTokenizer::new();
    let (tokens, state) = tokenizer.tokenize_line("```", LineState::Normal);
    debug!(?tokens, ?state, "fence start");
    assert_eq!(state, LineState::InString(StringKind::Backtick));
}

#[test]
fn test_token_kind_text_present() {
    setup_test_logging();
    info!("testing text fallback token");
    let tokenizer = JavaScriptTokenizer::javascript();
    let (tokens, state) = tokenizer.tokenize_line("$", LineState::Normal);
    debug!(?tokens, ?state, "text fallback");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Text));
}
