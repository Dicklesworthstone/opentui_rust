//! Buffer performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{Criterion, criterion_group, criterion_main};
use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use std::hint::black_box;

fn buffer_creation(c: &mut Criterion) {
    c.bench_function("buffer_new_80x24", |b| {
        b.iter(|| OptimizedBuffer::new(black_box(80), black_box(24)));
    });

    c.bench_function("buffer_new_200x50", |b| {
        b.iter(|| OptimizedBuffer::new(black_box(200), black_box(50)));
    });
}

fn buffer_clear(c: &mut Criterion) {
    let mut buffer = OptimizedBuffer::new(200, 50);

    c.bench_function("buffer_clear", |b| {
        b.iter(|| buffer.clear(black_box(Rgba::BLACK)))
    });
}

fn buffer_draw_text(c: &mut Criterion) {
    let mut buffer = OptimizedBuffer::new(200, 50);
    let style = Style::fg(Rgba::WHITE);

    c.bench_function("buffer_draw_text_short", |b| {
        b.iter(|| {
            buffer.draw_text(0, 0, black_box("Hello, World!"), style);
        })
    });

    c.bench_function("buffer_draw_text_long", |b| {
        let long_text = "x".repeat(100);
        b.iter(|| {
            buffer.draw_text(0, 0, black_box(&long_text), style);
        })
    });
}

fn buffer_cell_ops(c: &mut Criterion) {
    let mut buffer = OptimizedBuffer::new(200, 50);
    let cell = Cell::new('X', Style::fg(Rgba::RED));

    c.bench_function("buffer_set_cell", |b| {
        b.iter(|| {
            buffer.set(black_box(50), black_box(25), cell.clone());
        })
    });

    c.bench_function("buffer_get_cell", |b| {
        b.iter(|| {
            black_box(buffer.get(50, 25));
        })
    });
}

criterion_group!(
    benches,
    buffer_creation,
    buffer_clear,
    buffer_draw_text,
    buffer_cell_ops
);
criterion_main!(benches);
