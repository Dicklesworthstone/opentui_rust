//! Input parsing performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{Criterion, criterion_group, criterion_main};
use opentui::input::InputParser;
use opentui_rust as opentui;
use std::hint::black_box;

/// Benchmark key event parsing for various input sequences.
fn bench_key_event_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("key_parsing");

    // Common key sequences
    let key_sequences: &[(&[u8], &str)] = &[
        (b"a", "single_char"),
        (b"A", "single_uppercase"),
        (b"\x1b[A", "arrow_up"),
        (b"\x1b[B", "arrow_down"),
        (b"\x1b[C", "arrow_right"),
        (b"\x1b[D", "arrow_left"),
        (b"\x1b[1;5C", "ctrl_right"),
        (b"\x1b[1;2A", "shift_up"),
        (b"\x1bOP", "f1"),
        (b"\x1bOQ", "f2"),
        (b"\x1b[15~", "f5"),
        (b"\x1b[17~", "f6"),
        (b"\x1b[H", "home"),
        (b"\x1b[F", "end"),
        (b"\x1b[2~", "insert"),
        (b"\x1b[3~", "delete"),
        (b"\r", "enter"),
        (b"\t", "tab"),
        (b"\x7f", "backspace"),
        (b"\x1b", "escape"),
    ];

    for (seq, name) in key_sequences {
        group.bench_function(*name, |b| {
            let mut parser = InputParser::new();
            b.iter(|| parser.parse(black_box(*seq)));
        });
    }

    group.finish();
}

/// Benchmark mouse event parsing.
fn bench_mouse_event_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("mouse_parsing");

    // SGR mouse sequences
    let mouse_sequences: &[(&[u8], &str)] = &[
        (b"\x1b[<0;10;20M", "button_press"),
        (b"\x1b[<0;10;20m", "button_release"),
        (b"\x1b[<32;50;30M", "mouse_move"),
        (b"\x1b[<64;10;20M", "scroll_up"),
        (b"\x1b[<65;10;20M", "scroll_down"),
        (b"\x1b[<1;100;50M", "right_click"),
        (b"\x1b[<2;100;50M", "middle_click"),
    ];

    for (seq, name) in mouse_sequences {
        group.bench_function(*name, |b| {
            let mut parser = InputParser::new();
            b.iter(|| parser.parse(black_box(*seq)));
        });
    }

    group.finish();
}

/// Benchmark parsing a stream of mixed input.
fn bench_mixed_input_stream(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_input");

    // Simulate typing with occasional special keys
    let typing_stream: &[u8] = b"Hello, World!\x1b[C\x1b[D\r";

    group.bench_function("typing_with_navigation", |b| {
        let mut parser = InputParser::new();
        b.iter(|| {
            let mut remaining = black_box(typing_stream);
            let mut count = 0;
            while !remaining.is_empty() {
                match parser.parse(remaining) {
                    Ok((_, consumed)) => {
                        remaining = &remaining[consumed..];
                        count += 1;
                    }
                    Err(_) => break,
                }
            }
            count
        });
    });

    // Simulate rapid mouse movement
    let mouse_stream: &[u8] =
        b"\x1b[<32;10;10M\x1b[<32;11;10M\x1b[<32;12;10M\x1b[<32;13;10M\x1b[<32;14;10M";

    group.bench_function("rapid_mouse_moves", |b| {
        let mut parser = InputParser::new();
        b.iter(|| {
            let mut remaining = black_box(mouse_stream);
            let mut count = 0;
            while !remaining.is_empty() {
                match parser.parse(remaining) {
                    Ok((_, consumed)) => {
                        remaining = &remaining[consumed..];
                        count += 1;
                    }
                    Err(_) => break,
                }
            }
            count
        });
    });

    group.finish();
}

/// Benchmark batch input parsing performance.
fn bench_batch_parsing(c: &mut Criterion) {
    // Large batch of keyboard input (1000 characters)
    let keyboard_batch: Vec<u8> = (0..1000usize)
        .map(|i| b'a' + u8::try_from(i % 26).unwrap_or(0))
        .collect();

    c.bench_function("parse_1000_chars", |b| {
        let mut parser = InputParser::new();
        b.iter(|| {
            let mut remaining = black_box(keyboard_batch.as_slice());
            let mut count = 0;
            while !remaining.is_empty() {
                match parser.parse(remaining) {
                    Ok((_, consumed)) => {
                        remaining = &remaining[consumed..];
                        count += 1;
                    }
                    Err(_) => break,
                }
            }
            count
        });
    });

    // Mixed escape sequences
    let escape_batch: Vec<u8> = (0..100)
        .flat_map(|_| b"\x1b[A\x1b[B\x1b[C\x1b[D".iter().copied())
        .collect();

    c.bench_function("parse_400_arrows", |b| {
        let mut parser = InputParser::new();
        b.iter(|| {
            let mut remaining = black_box(escape_batch.as_slice());
            let mut count = 0;
            while !remaining.is_empty() {
                match parser.parse(remaining) {
                    Ok((_, consumed)) => {
                        remaining = &remaining[consumed..];
                        count += 1;
                    }
                    Err(_) => break,
                }
            }
            count
        });
    });
}

criterion_group!(
    benches,
    bench_key_event_parsing,
    bench_mouse_event_parsing,
    bench_mixed_input_stream,
    bench_batch_parsing
);
criterion_main!(benches);
