//! Renderer diff detection and ANSI output benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{Criterion, criterion_group, criterion_main};
use opentui::ansi::AnsiWriter;
use opentui::buffer::BoxStyle;
use opentui::renderer::BufferDiff;
use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use opentui_rust as opentui;
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

/// Benchmark reusable diff vs new allocation each time.
fn diff_reuse_vs_alloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_reuse");

    let a = OptimizedBuffer::new(80, 24);
    let mut b = OptimizedBuffer::new(80, 24);
    let style = Style::fg(Rgba::BLUE);
    // Scatter changes
    for y in 0..24 {
        for x in (0..80).step_by(3) {
            b.set(x, y, Cell::new('*', style));
        }
    }

    // Allocate new each time
    group.bench_function("diff_alloc_each_80x24", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a), black_box(&b)));
    });

    // Reuse pre-allocated diff
    group.bench_function("diff_reuse_80x24", |b_iter| {
        let mut diff = BufferDiff::with_capacity(1920 / 8);
        b_iter.iter(|| {
            diff.compute_into(black_box(&a), black_box(&b));
            black_box(&diff);
        });
    });

    // Larger buffer
    let a_large = OptimizedBuffer::new(200, 50);
    let mut b_large = OptimizedBuffer::new(200, 50);
    for y in 0..50 {
        for x in (0..200).step_by(3) {
            b_large.set(x, y, Cell::new('*', style));
        }
    }

    group.bench_function("diff_alloc_each_200x50", |b_iter| {
        b_iter.iter(|| BufferDiff::compute(black_box(&a_large), black_box(&b_large)));
    });

    group.bench_function("diff_reuse_200x50", |b_iter| {
        let mut diff = BufferDiff::with_capacity(10000 / 8);
        b_iter.iter(|| {
            diff.compute_into(black_box(&a_large), black_box(&b_large));
            black_box(&diff);
        });
    });

    group.finish();
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

/// Benchmark ANSI sequence generation.
fn ansi_generation(c: &mut Criterion) {
    use opentui::style::TextAttributes;

    let mut group = c.benchmark_group("ansi_generation");

    // Simple text output
    group.bench_function("ansi_simple_text", |b| {
        let mut output = Vec::with_capacity(256);

        b.iter(|| {
            output.clear();
            let mut writer = AnsiWriter::new(&mut output);
            writer.move_cursor(5, 10);
            writer.set_fg(Rgba::RED);
            writer.write_str("Hello, World!");
            writer.flush().unwrap();
            black_box(output.len());
        })
    });

    // Complex styled text with multiple lines
    group.bench_function("ansi_styled_multiline", |b| {
        let mut output = Vec::with_capacity(4096);

        b.iter(|| {
            output.clear();
            let mut writer = AnsiWriter::new(&mut output);
            for i in 0..10u8 {
                writer.move_cursor(u32::from(i), 0);
                writer.set_fg(Rgba::from_rgb_u8(i * 25, 100, 200));
                writer.set_bg(Rgba::from_rgb_u8(50, i * 25, 100));
                if i % 2 == 0 {
                    writer.set_attributes(TextAttributes::BOLD);
                }
                writer.write_str("Styled line of text here");
            }
            writer.flush().unwrap();
            black_box(output.len());
        })
    });

    // Cell-by-cell output (worst case)
    group.bench_function("ansi_cell_by_cell_80x24", |b| {
        let mut output = Vec::with_capacity(8192);

        b.iter(|| {
            output.clear();
            let mut writer = AnsiWriter::new(&mut output);
            for y in 0..24u32 {
                for x in 0..80u32 {
                    writer.move_cursor(y, x);
                    #[allow(clippy::cast_possible_truncation)]
                    writer.set_fg(Rgba::from_rgb_u8((x * 3) as u8, (y * 10) as u8, 128));
                    writer.write_str("X");
                }
            }
            writer.flush().unwrap();
            black_box(output.len());
        })
    });

    group.finish();
}

/// Benchmark full render cycle simulation.
fn render_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_cycle");

    // Complete frame preparation
    group.bench_function("prepare_frame_80x24", |b| {
        let mut front = OptimizedBuffer::new(80, 24);
        let mut back = OptimizedBuffer::new(80, 24);

        let bg = Rgba::from_rgb_u8(20, 20, 30);
        let text_style = Style::fg(Rgba::WHITE);
        let border_style = Style::fg(Rgba::from_rgb_u8(80, 80, 100));

        b.iter(|| {
            // Simulate drawing
            back.clear(bg);
            back.draw_text(10, 5, "Hello, World!", text_style);
            back.draw_box(5, 3, 70, 18, BoxStyle::double(border_style));

            // Calculate diff
            let diff = BufferDiff::compute(&front, &back);

            // Swap buffers
            std::mem::swap(&mut front, &mut back);

            black_box(diff);
        })
    });

    // Render cycle with ANSI output (simulated)
    group.bench_function("full_cycle_with_ansi_80x24", |b| {
        let mut front = OptimizedBuffer::new(80, 24);
        let mut back = OptimizedBuffer::new(80, 24);
        let mut output = Vec::with_capacity(8192);

        let bg = Rgba::from_rgb_u8(20, 20, 30);
        let text_style = Style::fg(Rgba::WHITE);

        b.iter(|| {
            // Drawing phase
            back.clear(bg);
            back.draw_text(10, 5, "Hello, World!", text_style);
            for y in 0..10u32 {
                back.draw_text(5, 8 + y, &format!("Line {y}"), text_style);
            }

            // Diff phase
            let diff = BufferDiff::compute(&front, &back);

            // ANSI generation phase
            output.clear();
            let mut writer = AnsiWriter::new(&mut output);
            if !diff.is_empty() {
                // Simulate writing changed content
                for y in 0..24u32 {
                    writer.move_cursor(y, 0);
                    if let Some(fg) = text_style.fg {
                        writer.set_fg(fg);
                    }
                    writer.write_str("                                        ");
                }
            }
            writer.flush().unwrap();

            // Swap buffers
            std::mem::swap(&mut front, &mut back);

            black_box(output.len());
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    diff_identical_buffers,
    diff_single_change,
    diff_row_change,
    diff_many_changes,
    diff_all_different,
    diff_reuse_vs_alloc,
    diff_should_full_redraw,
    ansi_generation,
    render_cycle
);
criterion_main!(benches);
