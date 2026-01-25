//! Syntax highlighting performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use opentui::highlight::languages::rust::RustTokenizer;
use opentui::highlight::tokenizer::{LineState, Tokenizer};
use opentui::highlight::{HighlightedBuffer, Theme};
use opentui::text::TextBuffer;
use std::hint::black_box;
use std::sync::Arc;

const SAMPLE_LINES: [&str; 4] = [
    "fn main() { println!(\"hello\"); }",
    "let x: HashMap<String, Vec<u32>> = HashMap::new();",
    "/// This is a doc comment with `code`",
    "#[derive(Debug, Clone, Serialize)]",
];

fn build_source(lines: usize) -> String {
    let line = "fn example() { let x = 42; println!(\"{x}\"); }\n";
    let mut text = String::with_capacity(lines * line.len());
    for _ in 0..lines {
        text.push_str(line);
    }
    text
}

fn bench_tokenize_line(c: &mut Criterion) {
    let tokenizer = RustTokenizer::new();
    let mut group = c.benchmark_group("highlight_tokenize_line");
    for (idx, line) in SAMPLE_LINES.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("line", idx), line, |b, input| {
            b.iter(|| tokenizer.tokenize_line(black_box(input), LineState::Normal));
        });
    }
    group.finish();
}

fn bench_tokenize_full_file(c: &mut Criterion) {
    let tokenizer = RustTokenizer::new();
    let source = include_str!("../src/lib.rs");
    c.bench_function("highlight_tokenize_full_file target<50ms", |b| {
        b.iter(|| tokenizer.tokenize(black_box(source)));
    });
}

fn bench_highlight_full(c: &mut Criterion) {
    let tokenizer: Arc<dyn Tokenizer> = Arc::new(RustTokenizer::new());
    let theme = Theme::dark();
    let source_small = build_source(1_000);
    let source_large = build_source(10_000);

    let mut group = c.benchmark_group("highlight_full_update");
    group.bench_function("1k_lines target<50ms", |b| {
        b.iter_batched(
            || {
                let buffer = TextBuffer::with_text(&source_small);
                let mut highlighted = HighlightedBuffer::new(buffer);
                highlighted.set_tokenizer(Some(tokenizer.clone()));
                highlighted.set_theme(theme.clone());
                highlighted
            },
            |mut highlighted| {
                highlighted.update_highlighting();
                black_box(highlighted);
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("10k_lines target<500ms", |b| {
        b.iter_batched(
            || {
                let buffer = TextBuffer::with_text(&source_large);
                let mut highlighted = HighlightedBuffer::new(buffer);
                highlighted.set_tokenizer(Some(tokenizer.clone()));
                highlighted.set_theme(theme.clone());
                highlighted
            },
            |mut highlighted| {
                highlighted.update_highlighting();
                black_box(highlighted);
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_incremental_update(c: &mut Criterion) {
    let tokenizer: Arc<dyn Tokenizer> = Arc::new(RustTokenizer::new());
    let source = build_source(2_000);
    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(&source));
    highlighted.set_tokenizer(Some(tokenizer));
    highlighted.set_theme(Theme::dark());
    highlighted.update_highlighting();

    let dirty_line = 100usize;
    let line_start = highlighted.rope().line_to_char(dirty_line);
    let edit_char = line_start + 4;
    let dirty_lines = [100usize, 101, 102, 103, 104];

    c.bench_function("highlight_incremental_single target<1ms", |b| {
        b.iter(|| {
            let rope = highlighted.rope_mut();
            rope.insert(edit_char, " ");
            rope.remove(edit_char..=edit_char);
            highlighted.mark_dirty(dirty_line);
            highlighted.update_highlighting();
        });
    });

    c.bench_function("highlight_incremental_multi", |b| {
        b.iter(|| {
            for line in dirty_lines {
                highlighted.mark_dirty(line);
            }
            highlighted.update_highlighting();
        });
    });
}

fn bench_theme_switch(c: &mut Criterion) {
    let tokenizer: Arc<dyn Tokenizer> = Arc::new(RustTokenizer::new());
    let source = build_source(1_000);
    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(&source));
    highlighted.set_tokenizer(Some(tokenizer));
    highlighted.set_theme(Theme::dark());
    highlighted.update_highlighting();

    let dark = Theme::dark();
    let light = Theme::light();
    let mut use_dark = false;

    c.bench_function("highlight_theme_switch target<100us", |b| {
        b.iter(|| {
            if use_dark {
                highlighted.set_theme(dark.clone());
            } else {
                highlighted.set_theme(light.clone());
            }
            highlighted.update_highlighting();
            use_dark = !use_dark;
        });
    });
}

fn bench_styled_line(c: &mut Criterion) {
    let tokenizer: Arc<dyn Tokenizer> = Arc::new(RustTokenizer::new());
    let source = build_source(1_000);
    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(&source));
    highlighted.set_tokenizer(Some(tokenizer));
    highlighted.set_theme(Theme::dark());
    highlighted.update_highlighting();

    c.bench_function("highlight_styled_line", |b| {
        b.iter(|| black_box(highlighted.styled_line(black_box(50))));
    });
}

fn bench_memory_estimate(c: &mut Criterion) {
    let tokenizer: Arc<dyn Tokenizer> = Arc::new(RustTokenizer::new());
    let source = build_source(1_000);
    let mut highlighted = HighlightedBuffer::new(TextBuffer::with_text(&source));
    highlighted.set_tokenizer(Some(tokenizer));
    highlighted.set_theme(Theme::dark());
    highlighted.update_highlighting();

    c.bench_function("highlight_memory_estimate", |b| {
        b.iter(|| {
            let mut token_count = 0usize;
            for line in 0..highlighted.len_lines() {
                token_count += highlighted.tokens_for_line(line).len();
            }
            let text_bytes = highlighted.buffer().len_bytes();
            black_box((token_count, text_bytes));
        });
    });
}

criterion_group!(
    benches,
    bench_tokenize_line,
    bench_tokenize_full_file,
    bench_highlight_full,
    bench_incremental_update,
    bench_theme_switch,
    bench_styled_line,
    bench_memory_estimate
);
criterion_main!(benches);
