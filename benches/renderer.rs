//! Renderer diff detection performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{Criterion, criterion_group, criterion_main};
use opentui::renderer::BufferDiff;
use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use std::hint::black_box;

fn diff_identical_buffers(c: &mut Criterion) {
    let a = OptimizedBuffer::new(80, 24);
    let b = OptimizedBuffer::new(80, 24);

    c.bench_function("diff_identical_80x24", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a), black_box(&b)));
    });

    let a_large = OptimizedBuffer::new(200, 50);
    let b_large = OptimizedBuffer::new(200, 50);

    c.bench_function("diff_identical_200x50", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a_large), black_box(&b_large)));
    });
}

fn diff_single_change(c: &mut Criterion) {
    let a = OptimizedBuffer::new(80, 24);
    let mut b = OptimizedBuffer::new(80, 24);
    b.set(40, 12, Cell::new('X', Style::fg(Rgba::RED)));

    c.bench_function("diff_single_change_80x24", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a), black_box(&b)));
    });
}

fn diff_row_change(c: &mut Criterion) {
    let a = OptimizedBuffer::new(80, 24);
    let mut b = OptimizedBuffer::new(80, 24);
    let style = Style::fg(Rgba::GREEN);
    for x in 0..80 {
        b.set(x, 12, Cell::new('=', style));
    }

    c.bench_function("diff_full_row_80x24", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a), black_box(&b)));
    });
}

fn diff_many_changes(c: &mut Criterion) {
    let a = OptimizedBuffer::new(80, 24);
    let mut b = OptimizedBuffer::new(80, 24);
    let style = Style::fg(Rgba::BLUE);
    // Scatter changes across buffer
    for y in 0..24 {
        for x in (0..80).step_by(3) {
            b.set(x, y, Cell::new('*', style));
        }
    }

    c.bench_function("diff_scattered_changes_80x24", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a), black_box(&b)));
    });
}

fn diff_all_different(c: &mut Criterion) {
    let a = OptimizedBuffer::new(80, 24);
    let mut b = OptimizedBuffer::new(80, 24);
    let style = Style::fg(Rgba::WHITE);
    for y in 0..24 {
        for x in 0..80 {
            b.set(x, y, Cell::new('#', style));
        }
    }

    c.bench_function("diff_all_different_80x24", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a), black_box(&b)));
    });

    // Large buffer
    let a_large = OptimizedBuffer::new(200, 50);
    let mut b_large = OptimizedBuffer::new(200, 50);
    for y in 0..50 {
        for x in 0..200 {
            b_large.set(x, y, Cell::new('#', style));
        }
    }

    c.bench_function("diff_all_different_200x50", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a_large), black_box(&b_large)));
    });
}

fn diff_should_full_redraw(c: &mut Criterion) {
    let a = OptimizedBuffer::new(80, 24);
    let mut b_few = OptimizedBuffer::new(80, 24);
    let mut b_many = OptimizedBuffer::new(80, 24);
    let style = Style::fg(Rgba::RED);

    // Few changes (< 50%)
    for y in 0..5 {
        for x in 0..80 {
            b_few.set(x, y, Cell::new('~', style));
        }
    }

    // Many changes (> 50%)
    for y in 0..20 {
        for x in 0..80 {
            b_many.set(x, y, Cell::new('~', style));
        }
    }

    c.bench_function("should_full_redraw_check", |b_iter| {
        let diff_few = BufferDiff::compute(&a, &b_few);
        let diff_many = BufferDiff::compute(&a, &b_many);
        let total = 80 * 24;
        b_iter.iter(|| {
            let r1 = diff_few.should_full_redraw(black_box(total));
            let r2 = diff_many.should_full_redraw(black_box(total));
            (r1, r2)
        });
    });
}

criterion_group!(
    benches,
    diff_identical_buffers,
    diff_single_change,
    diff_row_change,
    diff_many_changes,
    diff_all_different,
    diff_should_full_redraw
);
criterion_main!(benches);
