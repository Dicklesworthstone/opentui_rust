//! E2E tests for syntax highlighting pipeline.
//!
//! Run with:
//!   cargo test --test `highlight_e2e` -- --nocapture
//! With logging:
//!   `RUST_LOG=debug` cargo test --test `highlight_e2e` -- --nocapture
//!
//! CI: runs under the default `cargo test` job.

use std::cmp::max;
use std::fmt::Write;
use std::time::{Duration, Instant};

use opentui::highlight::languages::rust::RustTokenizer;
use opentui::highlight::{Token, TokenKind, TokenizerRegistry};
use opentui::{Cell, HighlightedBuffer, OptimizedBuffer, Rgba, TextBuffer, TextBufferView, Theme};
use tracing::{Level, debug, info, span};

const SAMPLE_RS: &str = include_str!("fixtures/sample.rs");
const SAMPLE_PY: &str = include_str!("fixtures/sample.py");
const SAMPLE_JSON: &str = include_str!("fixtures/sample.json");
const SAMPLE_TOML: &str = include_str!("fixtures/sample.toml");
const SAMPLE_MD: &str = include_str!("fixtures/sample.md");

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_test_writer()
        .try_init();
}

fn assert_tokens_well_formed(tokens: &[Token], line_len: usize) {
    let mut last_end = 0usize;
    for token in tokens {
        assert!(token.start <= token.end, "token has invalid span");
        assert!(token.end <= line_len, "token exceeds line length");
        assert!(token.start >= last_end, "token overlaps previous token");
        last_end = token.end;
    }
}

fn line_without_newline(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

#[test]
fn e2e_rust_file_highlighting() {
    init_logging();
    let span = span!(Level::INFO, "e2e_rust_file");
    let _enter = span.enter();

    info!("Starting Rust file E2E test");

    let source = SAMPLE_RS;
    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(source))
        .with_tokenizer(Box::new(RustTokenizer::new()))
        .with_theme(Theme::dark());

    let start = Instant::now();
    highlighted.update_highlighting();
    let initial_time = start.elapsed();
    info!(?initial_time, "Initial highlighting complete");

    let line_count = highlighted.len_lines();
    let check_lines = line_count.min(10);
    for line_idx in 0..check_lines {
        let line_text = highlighted.line(line_idx).unwrap_or_default();
        let line_trimmed = line_without_newline(&line_text);
        let tokens = highlighted.tokens_for_line(line_idx);

        debug!(
            line = line_idx,
            token_count = tokens.len(),
            preview = &line_trimmed[..line_trimmed.len().min(40)],
            "Line tokenized"
        );

        if !line_trimmed.is_empty() {
            assert!(!tokens.is_empty(), "expected tokens for line {line_idx}");
        }
        assert_tokens_well_formed(tokens, line_trimmed.len());
    }

    if line_count > 0 {
        let edit_line = line_count / 2;
        highlighted.mark_dirty(edit_line);
        let start = Instant::now();
        highlighted.update_highlighting();
        let incremental_time = start.elapsed();
        info!(?incremental_time, "Incremental update complete");

        let limit = max(initial_time.saturating_mul(2), Duration::from_millis(5));
        assert!(
            incremental_time <= limit,
            "incremental update should stay reasonable: initial={initial_time:?} incremental={incremental_time:?}"
        );
    }
}

#[test]
fn e2e_theme_switching() {
    init_logging();
    info!("Testing theme switching");

    let source = "fn main() { println!(\"hello\"); }";
    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(source))
        .with_tokenizer(Box::new(RustTokenizer::new()))
        .with_theme(Theme::dark());

    highlighted.update_highlighting();
    let keyword_pos = source.find("fn").expect("keyword position");
    let dark_style = highlighted.buffer().style_at(keyword_pos);

    highlighted.set_theme(Theme::light());
    highlighted.update_highlighting();
    let light_style = highlighted.buffer().style_at(keyword_pos);

    info!(?dark_style, ?light_style, "Theme styles captured");
    assert_ne!(dark_style, light_style, "Theme switch should change styles");
}

#[test]
fn e2e_render_highlighted_buffer() {
    init_logging();
    info!("Testing buffer rendering with highlighting");

    let source = "fn main() {\n    let x = 42;\n    println!(\"x = {}\", x);\n}\n";
    let theme = Theme::dark();
    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(source))
        .with_tokenizer(Box::new(RustTokenizer::new()))
        .with_theme(theme.clone());
    highlighted.update_highlighting();

    let view = TextBufferView::new(highlighted.buffer()).viewport(0, 0, 80, 24);
    let mut output = OptimizedBuffer::new(80, 24);
    output.clear(Rgba::from_hex("#1a1a2e").expect("valid color"));
    view.render_to(&mut output, 0, 0);

    let expected_style = theme
        .default_style()
        .merge(*theme.style_for(TokenKind::Keyword));
    let expected_cell = Cell::new('f', expected_style);

    let cell = *output.get(0, 0).expect("rendered cell");
    assert_eq!(cell, expected_cell, "Keyword cell should be highlighted");

    let non_empty_cells = output.cells().iter().filter(|c| !c.is_empty()).count();
    info!(non_empty_cells, "Rendered buffer populated");
    assert!(non_empty_cells > 20, "Buffer should have content");
}

#[test]
fn e2e_fixture_highlighting_smoke() {
    init_logging();
    info!("Testing fixture highlighting via registry");

    let registry = TokenizerRegistry::with_builtins();
    let fixtures = [
        ("rs", SAMPLE_RS),
        ("py", SAMPLE_PY),
        ("json", SAMPLE_JSON),
        ("toml", SAMPLE_TOML),
        ("md", SAMPLE_MD),
    ];

    for (ext, source) in fixtures {
        let tokenizer = registry
            .for_extension_shared(ext)
            .unwrap_or_else(|| panic!("Missing tokenizer for {ext}"));
        let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(source));
        highlighted.set_tokenizer(Some(tokenizer));
        highlighted.set_theme(Theme::dark());
        highlighted.update_highlighting();

        let first_line = source.lines().next().unwrap_or("");
        let tokens = highlighted.tokens_for_line(0);
        debug!(ext, token_count = tokens.len(), "Tokenized fixture");
        if !first_line.trim().is_empty() {
            assert!(!tokens.is_empty(), "{ext} should produce tokens");
        }
        assert_tokens_well_formed(tokens, first_line.len());
    }
}

#[test]
fn e2e_performance_regression() {
    init_logging();
    info!("Performance regression test");

    let mut source = String::with_capacity(32 * 1000);
    for i in 0..1000 {
        let _ = writeln!(&mut source, "fn func_{i}() {{ let x = {i}; }}");
    }

    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(&source))
        .with_tokenizer(Box::new(RustTokenizer::new()))
        .with_theme(Theme::dark());

    let start = Instant::now();
    highlighted.update_highlighting();
    let elapsed = start.elapsed();
    info!(?elapsed, lines = 1000, "Initial highlight timing");

    let max_initial = Duration::from_secs(2);
    assert!(
        elapsed <= max_initial,
        "1000 lines should highlight within {max_initial:?}, took {elapsed:?}"
    );

    highlighted.mark_dirty(500);
    let start = Instant::now();
    highlighted.update_highlighting();
    let incremental = start.elapsed();
    info!(?incremental, "Incremental update timing");

    let max_incremental = Duration::from_millis(200);
    assert!(
        incremental <= max_incremental,
        "Incremental update should be within {max_incremental:?}, took {incremental:?}"
    );
}

#[test]
fn e2e_malformed_input_handling() {
    init_logging();
    info!("Testing malformed input handling");

    let malformed_inputs = [
        "fn main() { /* unterminated comment",
        "let s = \"unterminated string",
        "let x = 0x",
        "fn ()",
    ];

    for input in malformed_inputs {
        info!(input, "Testing malformed input");
        let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(input))
            .with_tokenizer(Box::new(RustTokenizer::new()))
            .with_theme(Theme::dark());

        highlighted.update_highlighting();
        let tokens = highlighted.tokens_for_line(0);
        debug!(?tokens, "Tokens for malformed input");
        assert_tokens_well_formed(tokens, input.len());
    }
}
